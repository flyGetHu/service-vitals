//! Service Vitals 主程序入口
//!
//! 跨平台服务健康监控工具

use service_vitals::app;

#[tokio::main]
async fn main() {
    // 委托给应用程序模块处理
    if let Err(e) = app::main().await {
        eprintln!("应用程序启动失败: {}", e);
        std::process::exit(1);
    }
}
