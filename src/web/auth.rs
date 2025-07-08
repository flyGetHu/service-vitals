//! 认证模块
//!
//! 提供API密钥认证和访问控制

use super::{ApiError, WebServerState};
use std::convert::Infallible;
use std::sync::Arc;
use warp::{http::HeaderMap, Filter, Rejection};

/// API密钥头部名称
pub const API_KEY_HEADER: &str = "X-API-Key";

/// API密钥查询参数名称
pub const API_KEY_QUERY: &str = "api_key";

/// 认证错误类型
#[derive(Debug)]
pub enum AuthError {
    /// 缺少API密钥
    MissingApiKey,
    /// 无效的API密钥
    InvalidApiKey,
    /// 认证被禁用
    AuthDisabled,
}

impl warp::reject::Reject for AuthError {}

/// 创建认证过滤器
pub fn auth_filter(
    state: Arc<WebServerState>,
) -> impl Filter<Extract = (), Error = Rejection> + Clone {
    warp::any()
        .and(warp::header::headers_cloned())
        .and(warp::query::raw().or(warp::any().map(|| String::new())).unify())
        .and(with_state(state))
        .and_then(authenticate)
        .untuple_one()
}

/// 状态注入过滤器
fn with_state(
    state: Arc<WebServerState>,
) -> impl Filter<Extract = (Arc<WebServerState>,), Error = Infallible> + Clone {
    warp::any().map(move || state.clone())
}

/// 认证处理函数
async fn authenticate(
    headers: HeaderMap,
    query: String,
    state: Arc<WebServerState>,
) -> Result<(), Rejection> {
    // 如果禁用了认证，直接通过
    if state.config.disable_auth {
        return Ok(());
    }

    // 如果没有配置API密钥，也直接通过（但会在配置验证时警告）
    let expected_api_key = match &state.config.api_key {
        Some(key) => key,
        None => return Ok(()),
    };

    // 从头部获取API密钥
    if let Some(api_key) = get_api_key_from_header(&headers) {
        if api_key == *expected_api_key {
            return Ok(());
        } else {
            return Err(warp::reject::custom(AuthError::InvalidApiKey));
        }
    }

    // 从查询参数获取API密钥
    if let Some(api_key) = get_api_key_from_query(&query) {
        if api_key == *expected_api_key {
            return Ok(());
        } else {
            return Err(warp::reject::custom(AuthError::InvalidApiKey));
        }
    }

    // 没有找到API密钥
    Err(warp::reject::custom(AuthError::MissingApiKey))
}

/// 从HTTP头部获取API密钥
fn get_api_key_from_header(headers: &HeaderMap) -> Option<String> {
    headers
        .get(API_KEY_HEADER)
        .and_then(|value| value.to_str().ok())
        .map(|s| s.to_string())
}

/// 从查询参数获取API密钥
fn get_api_key_from_query(query: &str) -> Option<String> {
    if query.is_empty() {
        return None;
    }

    for pair in query.split('&') {
        if let Some((key, value)) = pair.split_once('=') {
            if key == API_KEY_QUERY {
                return Some(urlencoding::decode(value).ok()?.into_owned());
            }
        }
    }

    None
}

/// 创建认证错误响应
pub async fn handle_auth_error(err: Rejection) -> Result<impl warp::Reply, Infallible> {
    if let Some(auth_error) = err.find::<AuthError>() {
        let (code, message) = match auth_error {
            AuthError::MissingApiKey => (401, "缺少API密钥，请在X-API-Key头部或api_key查询参数中提供"),
            AuthError::InvalidApiKey => (401, "无效的API密钥"),
            AuthError::AuthDisabled => (200, "认证已禁用"),
        };

        let error = ApiError::new(code, message.to_string());
        let json = warp::reply::json(&error);
        
        Ok(warp::reply::with_status(json, warp::http::StatusCode::from_u16(code).unwrap()))
    } else if err.is_not_found() {
        let error = ApiError::new(404, "端点未找到".to_string());
        let json = warp::reply::json(&error);
        Ok(warp::reply::with_status(json, warp::http::StatusCode::NOT_FOUND))
    } else if let Some(api_error) = err.find::<ApiError>() {
        let json = warp::reply::json(api_error);
        let status_code = warp::http::StatusCode::from_u16(api_error.code).unwrap_or(warp::http::StatusCode::INTERNAL_SERVER_ERROR);
        Ok(warp::reply::with_status(json, status_code))
    } else {
        let error = ApiError::new(500, "内部服务器错误".to_string());
        let json = warp::reply::json(&error);
        Ok(warp::reply::with_status(json, warp::http::StatusCode::INTERNAL_SERVER_ERROR))
    }
}

/// 生成安全的API密钥
pub fn generate_api_key() -> String {
    use rand::Rng;
    const CHARSET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";
    const KEY_LENGTH: usize = 32;

    let mut rng = rand::thread_rng();
    (0..KEY_LENGTH)
        .map(|_| {
            let idx = rng.gen_range(0..CHARSET.len());
            CHARSET[idx] as char
        })
        .collect()
}

/// 验证API密钥格式
pub fn validate_api_key(api_key: &str) -> bool {
    // API密钥应该至少16个字符，只包含字母数字字符
    if api_key.len() < 16 {
        return false;
    }

    api_key.chars().all(|c| c.is_alphanumeric())
}

#[cfg(test)]
mod tests {
    use super::*;
    use warp::http::HeaderValue;

    #[test]
    fn test_get_api_key_from_header() {
        let mut headers = HeaderMap::new();
        headers.insert(API_KEY_HEADER, HeaderValue::from_static("test-api-key"));

        let api_key = get_api_key_from_header(&headers);
        assert_eq!(api_key, Some("test-api-key".to_string()));
    }

    #[test]
    fn test_get_api_key_from_query() {
        let query = "param1=value1&api_key=test-key&param2=value2";
        let api_key = get_api_key_from_query(query);
        assert_eq!(api_key, Some("test-key".to_string()));
    }

    #[test]
    fn test_get_api_key_from_query_encoded() {
        let query = "api_key=test%2Dkey"; // test-key encoded
        let api_key = get_api_key_from_query(query);
        assert_eq!(api_key, Some("test-key".to_string()));
    }

    #[test]
    fn test_get_api_key_from_query_not_found() {
        let query = "param1=value1&param2=value2";
        let api_key = get_api_key_from_query(query);
        assert_eq!(api_key, None);
    }

    #[test]
    fn test_generate_api_key() {
        let key1 = generate_api_key();
        let key2 = generate_api_key();
        
        assert_eq!(key1.len(), 32);
        assert_eq!(key2.len(), 32);
        assert_ne!(key1, key2); // 应该生成不同的密钥
        assert!(key1.chars().all(|c| c.is_alphanumeric()));
    }

    #[test]
    fn test_validate_api_key() {
        assert!(validate_api_key("abcdefghijklmnop1234567890"));
        assert!(!validate_api_key("short")); // 太短
        assert!(!validate_api_key("contains-special-chars!")); // 包含特殊字符
        assert!(validate_api_key("1234567890abcdef")); // 最小长度
    }
}
