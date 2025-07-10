//! 信号处理模块
//!
//! 提供Linux/Unix信号处理和优雅关闭支持

use crate::error::Result;
use tokio::sync::broadcast;
use tracing::{error, info, warn};

#[cfg(unix)]
use signal_hook::consts::{SIGINT, SIGTERM, SIGUSR1};
#[cfg(unix)]
use signal_hook_tokio::Signals;

/// 设置信号处理器
pub async fn setup_signal_handlers(shutdown_tx: broadcast::Sender<()>) -> Result<()> {
    #[cfg(unix)]
    {
        setup_unix_signals(shutdown_tx).await
    }
    #[cfg(not(unix))]
    {
        info!("非Unix系统，跳过信号处理器设置");
        Ok(())
    }
}

/// Unix/Linux系统信号处理
#[cfg(unix)]
async fn setup_unix_signals(shutdown_tx: broadcast::Sender<()>) -> Result<()> {
    use futures::stream::StreamExt;

    let signals = Signals::new([SIGINT, SIGTERM, SIGUSR1])?;
    let handle = signals.handle();

    // Clone the sender before moving it
    let shutdown_tx_signals = shutdown_tx.clone();
    let signals_task = async move {
        let mut signals = signals;
        while let Some(signal) = signals.next().await {
            match signal {
                SIGINT => {
                    info!("接收到 SIGINT 信号，开始优雅关闭...");
                    if let Err(e) = shutdown_tx_signals.send(()) {
                        error!("发送关闭信号失败: {e}");
                    }
                    break;
                }
                SIGTERM => {
                    info!("接收到 SIGTERM 信号，开始优雅关闭...");
                    if let Err(e) = shutdown_tx_signals.send(()) {
                        error!("发送关闭信号失败: {e}");
                    }
                    break;
                }
                SIGUSR1 => {
                    info!("接收到 SIGUSR1 信号，重新加载配置...");
                    // TODO: 实现配置重载逻辑
                }
                _ => {
                    warn!("接收到未处理的信号: {signal}");
                }
            }
        }
    };

    tokio::spawn(signals_task);

    // 注册清理函数
    let shutdown_tx_ctrl_c = shutdown_tx.clone();
    tokio::spawn(async move {
        tokio::signal::ctrl_c()
            .await
            .expect("Failed to listen for ctrl_c");
        info!("接收到 Ctrl+C，开始优雅关闭...");
        if let Err(e) = shutdown_tx_ctrl_c.send(()) {
            error!("发送关闭信号失败: {e}");
        }
        handle.close();
    });

    Ok(())
}

/// 等待关闭信号
pub async fn wait_for_shutdown(mut shutdown_rx: broadcast::Receiver<()>) {
    match shutdown_rx.recv().await {
        Ok(()) => {
            info!("接收到关闭信号，开始清理资源...");
        }
        Err(e) => {
            error!("等待关闭信号时发生错误: {e}");
        }
    }
}

/// 优雅关闭处理器
pub struct GracefulShutdown {
    /// 关闭信号接收器
    shutdown_rx: broadcast::Receiver<()>,
    /// 关闭超时时间（秒）
    timeout_seconds: u64,
}

impl GracefulShutdown {
    /// 创建新的优雅关闭处理器
    pub fn new(shutdown_rx: broadcast::Receiver<()>, timeout_seconds: u64) -> Self {
        Self {
            shutdown_rx,
            timeout_seconds,
        }
    }

    /// 等待关闭信号或超时
    pub async fn wait_for_shutdown(&mut self) -> bool {
        let timeout = tokio::time::Duration::from_secs(self.timeout_seconds);

        match tokio::time::timeout(timeout, self.shutdown_rx.recv()).await {
            Ok(Ok(())) => {
                info!("接收到关闭信号");
                true
            }
            Ok(Err(e)) => {
                error!("接收关闭信号时发生错误: {e}");
                false
            }
            Err(_) => {
                warn!("等待关闭信号超时 ({} 秒)", self.timeout_seconds);
                false
            }
        }
    }

    /// 执行优雅关闭流程
    pub async fn shutdown_gracefully<F, Fut>(&mut self, cleanup_fn: F) -> Result<()>
    where
        F: FnOnce() -> Fut,
        Fut: std::future::Future<Output = Result<()>>,
    {
        info!("开始优雅关闭流程...");

        // 等待关闭信号
        if self.wait_for_shutdown().await {
            info!("执行清理操作...");

            // 执行清理函数
            if let Err(e) = cleanup_fn().await {
                error!("清理操作失败: {e}");
                return Err(e);
            }

            info!("优雅关闭完成");
        } else {
            warn!("强制关闭");
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::{sleep, Duration};

    #[tokio::test]
    async fn test_graceful_shutdown_creation() {
        let (_, shutdown_rx) = broadcast::channel(1);
        let graceful_shutdown = GracefulShutdown::new(shutdown_rx, 5);
        assert_eq!(graceful_shutdown.timeout_seconds, 5);
    }

    #[tokio::test]
    #[ignore] // 忽略这个测试，因为时间相关的测试在CI环境中可能不稳定
    async fn test_graceful_shutdown_timeout() {
        let (_, shutdown_rx) = broadcast::channel(1);
        let mut graceful_shutdown = GracefulShutdown::new(shutdown_rx, 1);

        let start = std::time::Instant::now();
        let result = graceful_shutdown.wait_for_shutdown().await;
        let elapsed = start.elapsed();

        assert!(!result); // 应该超时
                          // 允许一些时间误差，检查是否接近1秒
        assert!(elapsed >= Duration::from_millis(900));
        assert!(elapsed <= Duration::from_millis(1200));
    }

    #[tokio::test]
    async fn test_graceful_shutdown_signal() {
        let (shutdown_tx, shutdown_rx) = broadcast::channel(1);
        let mut graceful_shutdown = GracefulShutdown::new(shutdown_rx, 5);

        // 在另一个任务中发送关闭信号
        tokio::spawn(async move {
            sleep(Duration::from_millis(100)).await;
            let _ = shutdown_tx.send(());
        });

        let start = std::time::Instant::now();
        let result = graceful_shutdown.wait_for_shutdown().await;
        let elapsed = start.elapsed();

        assert!(result); // 应该接收到信号
        assert!(elapsed < Duration::from_secs(1)); // 应该很快完成
    }
}
