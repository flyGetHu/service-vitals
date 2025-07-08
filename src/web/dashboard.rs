//! Web仪表板模块
//!
//! 提供HTML/CSS/JS前端界面

use super::WebServerState;
use std::convert::Infallible;
use std::sync::Arc;
use warp::{Filter, Rejection, Reply};

/// 创建仪表板路由
pub fn create_dashboard_routes(
    state: Arc<WebServerState>,
) -> impl Filter<Extract = impl Reply, Error = Rejection> + Clone {
    let index_route = warp::path::end()
        .and(warp::get())
        .and_then(serve_index);

    let dashboard_route = warp::path("dashboard")
        .and(warp::path::end())
        .and(warp::get())
        .and_then(serve_dashboard);

    let static_routes = warp::path("static")
        .and(warp::path("dashboard.css"))
        .and(warp::path::end())
        .and(warp::get())
        .and_then(serve_css);

    let js_routes = warp::path("static")
        .and(warp::path("dashboard.js"))
        .and(warp::path::end())
        .and(warp::get())
        .and_then(serve_js);

    index_route.or(dashboard_route).or(static_routes).or(js_routes)
}

/// 提供首页
async fn serve_index() -> Result<impl Reply, Infallible> {
    Ok(warp::reply::html(INDEX_HTML))
}

/// 提供仪表板页面
async fn serve_dashboard() -> Result<impl Reply, Infallible> {
    Ok(warp::reply::html(DASHBOARD_HTML))
}

/// 提供CSS文件
async fn serve_css() -> Result<impl Reply, Infallible> {
    Ok(warp::reply::with_header(
        DASHBOARD_CSS,
        "content-type",
        "text/css",
    ))
}

/// 提供JavaScript文件
async fn serve_js() -> Result<impl Reply, Infallible> {
    Ok(warp::reply::with_header(
        DASHBOARD_JS,
        "content-type",
        "application/javascript",
    ))
}

/// 首页HTML
const INDEX_HTML: &str = r#"
<!DOCTYPE html>
<html lang="zh-CN">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Service Vitals</title>
    <style>
        body {
            font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
            margin: 0;
            padding: 40px;
            background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
            min-height: 100vh;
            display: flex;
            align-items: center;
            justify-content: center;
        }
        .container {
            background: white;
            padding: 40px;
            border-radius: 12px;
            box-shadow: 0 10px 30px rgba(0,0,0,0.2);
            text-align: center;
            max-width: 500px;
        }
        h1 {
            color: #333;
            margin-bottom: 20px;
            font-size: 2.5em;
        }
        .subtitle {
            color: #666;
            margin-bottom: 30px;
            font-size: 1.2em;
        }
        .btn {
            display: inline-block;
            padding: 12px 24px;
            background: #667eea;
            color: white;
            text-decoration: none;
            border-radius: 6px;
            font-weight: 500;
            transition: background 0.3s;
        }
        .btn:hover {
            background: #5a6fd8;
        }
        .features {
            margin-top: 30px;
            text-align: left;
        }
        .feature {
            margin: 10px 0;
            color: #555;
        }
        .feature::before {
            content: "✓ ";
            color: #4CAF50;
            font-weight: bold;
        }
    </style>
</head>
<body>
    <div class="container">
        <h1>🔍 Service Vitals</h1>
        <p class="subtitle">服务健康监控和告警系统</p>
        
        <a href="/dashboard" class="btn">进入监控仪表板</a>
        
        <div class="features">
            <div class="feature">实时服务状态监控</div>
            <div class="feature">响应时间统计</div>
            <div class="feature">自动故障告警</div>
            <div class="feature">RESTful API接口</div>
            <div class="feature">Prometheus指标导出</div>
        </div>
    </div>
</body>
</html>
"#;

/// 仪表板HTML
const DASHBOARD_HTML: &str = r#"
<!DOCTYPE html>
<html lang="zh-CN">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Service Vitals - 监控仪表板</title>
    <link rel="stylesheet" href="/static/dashboard.css">
</head>
<body>
    <div class="header">
        <h1>🔍 Service Vitals 监控仪表板</h1>
        <div class="header-info">
            <span id="last-update">最后更新: --</span>
            <button id="refresh-btn" onclick="refreshData()">🔄 刷新</button>
        </div>
    </div>

    <div class="container">
        <div class="summary-cards">
            <div class="card">
                <div class="card-title">总服务数</div>
                <div class="card-value" id="total-services">--</div>
            </div>
            <div class="card">
                <div class="card-title">健康服务</div>
                <div class="card-value healthy" id="healthy-services">--</div>
            </div>
            <div class="card">
                <div class="card-title">异常服务</div>
                <div class="card-value unhealthy" id="unhealthy-services">--</div>
            </div>
            <div class="card">
                <div class="card-title">禁用服务</div>
                <div class="card-value disabled" id="disabled-services">--</div>
            </div>
        </div>

        <div class="services-section">
            <h2>服务状态详情</h2>
            <div class="services-table-container">
                <table class="services-table" id="services-table">
                    <thead>
                        <tr>
                            <th>服务名称</th>
                            <th>状态</th>
                            <th>状态码</th>
                            <th>响应时间</th>
                            <th>最后检测</th>
                            <th>连续失败</th>
                            <th>错误信息</th>
                        </tr>
                    </thead>
                    <tbody id="services-tbody">
                        <tr>
                            <td colspan="7" class="loading">加载中...</td>
                        </tr>
                    </tbody>
                </table>
            </div>
        </div>

        <div class="api-section">
            <h2>API 端点</h2>
            <div class="api-endpoints">
                <div class="endpoint">
                    <code>GET /api/v1/status</code>
                    <span>获取所有服务状态</span>
                </div>
                <div class="endpoint">
                    <code>GET /api/v1/status/{service_name}</code>
                    <span>获取特定服务状态</span>
                </div>
                <div class="endpoint">
                    <code>GET /api/v1/config</code>
                    <span>获取配置信息</span>
                </div>
                <div class="endpoint">
                    <code>GET /api/v1/health</code>
                    <span>健康检查</span>
                </div>
                <div class="endpoint">
                    <code>GET /metrics</code>
                    <span>Prometheus指标</span>
                </div>
            </div>
        </div>
    </div>

    <script src="/static/dashboard.js"></script>
</body>
</html>
"#;

/// 仪表板CSS
const DASHBOARD_CSS: &str = r#"
* {
    margin: 0;
    padding: 0;
    box-sizing: border-box;
}

body {
    font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
    background-color: #f5f5f5;
    color: #333;
    line-height: 1.6;
}

.header {
    background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
    color: white;
    padding: 20px 0;
    box-shadow: 0 2px 10px rgba(0,0,0,0.1);
}

.header h1 {
    text-align: center;
    margin-bottom: 10px;
    font-size: 2em;
}

.header-info {
    text-align: center;
    display: flex;
    justify-content: center;
    align-items: center;
    gap: 20px;
}

#refresh-btn {
    background: rgba(255,255,255,0.2);
    border: 1px solid rgba(255,255,255,0.3);
    color: white;
    padding: 8px 16px;
    border-radius: 4px;
    cursor: pointer;
    transition: background 0.3s;
}

#refresh-btn:hover {
    background: rgba(255,255,255,0.3);
}

.container {
    max-width: 1200px;
    margin: 0 auto;
    padding: 20px;
}

.summary-cards {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(200px, 1fr));
    gap: 20px;
    margin-bottom: 30px;
}

.card {
    background: white;
    padding: 20px;
    border-radius: 8px;
    box-shadow: 0 2px 10px rgba(0,0,0,0.1);
    text-align: center;
}

.card-title {
    font-size: 0.9em;
    color: #666;
    margin-bottom: 10px;
}

.card-value {
    font-size: 2em;
    font-weight: bold;
    color: #333;
}

.card-value.healthy {
    color: #4CAF50;
}

.card-value.unhealthy {
    color: #f44336;
}

.card-value.disabled {
    color: #ff9800;
}

.services-section {
    background: white;
    padding: 20px;
    border-radius: 8px;
    box-shadow: 0 2px 10px rgba(0,0,0,0.1);
    margin-bottom: 30px;
}

.services-section h2 {
    margin-bottom: 20px;
    color: #333;
}

.services-table-container {
    overflow-x: auto;
}

.services-table {
    width: 100%;
    border-collapse: collapse;
    font-size: 0.9em;
}

.services-table th,
.services-table td {
    padding: 12px;
    text-align: left;
    border-bottom: 1px solid #eee;
}

.services-table th {
    background-color: #f8f9fa;
    font-weight: 600;
    color: #555;
}

.services-table tr:hover {
    background-color: #f8f9fa;
}

.status-badge {
    padding: 4px 8px;
    border-radius: 4px;
    font-size: 0.8em;
    font-weight: 500;
}

.status-up {
    background-color: #d4edda;
    color: #155724;
}

.status-down {
    background-color: #f8d7da;
    color: #721c24;
}

.status-unknown {
    background-color: #fff3cd;
    color: #856404;
}

.loading {
    text-align: center;
    color: #666;
    font-style: italic;
}

.api-section {
    background: white;
    padding: 20px;
    border-radius: 8px;
    box-shadow: 0 2px 10px rgba(0,0,0,0.1);
}

.api-section h2 {
    margin-bottom: 20px;
    color: #333;
}

.api-endpoints {
    display: grid;
    gap: 10px;
}

.endpoint {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 10px;
    background-color: #f8f9fa;
    border-radius: 4px;
    border-left: 4px solid #667eea;
}

.endpoint code {
    background-color: #e9ecef;
    padding: 4px 8px;
    border-radius: 4px;
    font-family: 'Monaco', 'Menlo', monospace;
    font-size: 0.9em;
}

.endpoint span {
    color: #666;
    font-size: 0.9em;
}

@media (max-width: 768px) {
    .container {
        padding: 10px;
    }
    
    .summary-cards {
        grid-template-columns: repeat(2, 1fr);
    }
    
    .header-info {
        flex-direction: column;
        gap: 10px;
    }
    
    .endpoint {
        flex-direction: column;
        align-items: flex-start;
        gap: 5px;
    }
}
"#;

/// 仪表板JavaScript
const DASHBOARD_JS: &str = r#"
// 全局变量
let refreshInterval;
const REFRESH_INTERVAL = 30000; // 30秒

// 页面加载完成后初始化
document.addEventListener('DOMContentLoaded', function() {
    refreshData();
    startAutoRefresh();
});

// 刷新数据
async function refreshData() {
    try {
        const response = await fetch('/api/v1/status');
        const result = await response.json();
        
        if (result.success) {
            updateSummaryCards(result.data);
            updateServicesTable(result.data.services);
            updateLastUpdateTime();
        } else {
            showError('获取数据失败: ' + (result.error || '未知错误'));
        }
    } catch (error) {
        showError('网络错误: ' + error.message);
    }
}

// 更新汇总卡片
function updateSummaryCards(data) {
    document.getElementById('total-services').textContent = data.total_services;
    document.getElementById('healthy-services').textContent = data.healthy_services;
    document.getElementById('unhealthy-services').textContent = data.unhealthy_services;
    document.getElementById('disabled-services').textContent = data.disabled_services;
}

// 更新服务表格
function updateServicesTable(services) {
    const tbody = document.getElementById('services-tbody');
    
    if (services.length === 0) {
        tbody.innerHTML = '<tr><td colspan="7" class="loading">暂无服务数据</td></tr>';
        return;
    }
    
    tbody.innerHTML = services.map(service => `
        <tr>
            <td><strong>${escapeHtml(service.name)}</strong></td>
            <td><span class="status-badge ${getStatusClass(service.status)}">${getStatusText(service.status)}</span></td>
            <td>${service.status_code || '--'}</td>
            <td>${service.response_time_ms ? service.response_time_ms + 'ms' : '--'}</td>
            <td>${formatDateTime(service.last_check)}</td>
            <td>${service.consecutive_failures}</td>
            <td>${service.error_message ? escapeHtml(service.error_message) : '--'}</td>
        </tr>
    `).join('');
}

// 获取状态样式类
function getStatusClass(status) {
    switch (status.toLowerCase()) {
        case 'up': return 'status-up';
        case 'down': return 'status-down';
        default: return 'status-unknown';
    }
}

// 获取状态文本
function getStatusText(status) {
    switch (status.toLowerCase()) {
        case 'up': return '✅ 正常';
        case 'down': return '❌ 异常';
        case 'degraded': return '⚠️ 降级';
        default: return '❓ 未知';
    }
}

// 格式化日期时间
function formatDateTime(dateString) {
    if (!dateString) return '--';
    
    const date = new Date(dateString);
    const now = new Date();
    const diff = now - date;
    
    // 如果是最近1分钟内
    if (diff < 60000) {
        return '刚刚';
    }
    
    // 如果是最近1小时内
    if (diff < 3600000) {
        const minutes = Math.floor(diff / 60000);
        return `${minutes}分钟前`;
    }
    
    // 如果是今天
    if (date.toDateString() === now.toDateString()) {
        return date.toLocaleTimeString('zh-CN', { hour: '2-digit', minute: '2-digit' });
    }
    
    // 其他情况显示完整日期时间
    return date.toLocaleString('zh-CN');
}

// 更新最后更新时间
function updateLastUpdateTime() {
    const now = new Date();
    document.getElementById('last-update').textContent = 
        `最后更新: ${now.toLocaleTimeString('zh-CN')}`;
}

// 显示错误信息
function showError(message) {
    const tbody = document.getElementById('services-tbody');
    tbody.innerHTML = `<tr><td colspan="7" style="color: #f44336; text-align: center;">${escapeHtml(message)}</td></tr>`;
}

// HTML转义
function escapeHtml(text) {
    const div = document.createElement('div');
    div.textContent = text;
    return div.innerHTML;
}

// 开始自动刷新
function startAutoRefresh() {
    refreshInterval = setInterval(refreshData, REFRESH_INTERVAL);
}

// 停止自动刷新
function stopAutoRefresh() {
    if (refreshInterval) {
        clearInterval(refreshInterval);
        refreshInterval = null;
    }
}

// 页面失去焦点时停止刷新，获得焦点时恢复
document.addEventListener('visibilitychange', function() {
    if (document.hidden) {
        stopAutoRefresh();
    } else {
        refreshData();
        startAutoRefresh();
    }
});
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_html_constants_not_empty() {
        assert!(!INDEX_HTML.is_empty());
        assert!(!DASHBOARD_HTML.is_empty());
        assert!(!DASHBOARD_CSS.is_empty());
        assert!(!DASHBOARD_JS.is_empty());
    }

    #[test]
    fn test_html_contains_expected_elements() {
        assert!(INDEX_HTML.contains("Service Vitals"));
        assert!(DASHBOARD_HTML.contains("监控仪表板"));
        assert!(DASHBOARD_CSS.contains(".services-table"));
        assert!(DASHBOARD_JS.contains("refreshData"));
    }
}
