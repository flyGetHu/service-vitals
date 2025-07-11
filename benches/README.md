# 基准测试

本目录包含 Service Vitals 项目的基准测试，用于测量关键组件的性能。

## 可用的基准测试

### 1. 健康检测器基准测试 (`health_checker.rs`)

测试健康检测相关组件的性能：

- **health_result_creation**: 测试健康结果对象的创建性能
- **service_config_creation**: 测试服务配置对象的创建性能
- **health_result_serialization**: 测试健康结果JSON序列化性能
- **health_result_deserialization**: 测试健康结果JSON反序列化性能

### 2. 配置处理基准测试 (`config_processing.rs`)

测试配置相关操作的性能：

- **config_creation**: 测试配置对象的创建性能
- **config_serialization**: 测试配置TOML序列化性能
- **config_deserialization**: 测试配置TOML反序列化性能
- **config_validation**: 测试配置验证性能

### 3. 通知处理基准测试 (`notification.rs`)

测试通知模板渲染的性能：

- **template_rendering**: 测试简单模板渲染性能
- **template_rendering_complex**: 测试复杂模板渲染性能
- **template_creation**: 测试模板创建性能
- **health_result_to_notification_data**: 测试健康结果转换为通知数据的性能

## 运行基准测试

### 运行所有基准测试

```bash
cargo bench
```

### 运行特定基准测试

```bash
# 运行健康检测器基准测试
cargo bench --bench health_checker

# 运行配置处理基准测试
cargo bench --bench config_processing

# 运行通知处理基准测试
cargo bench --bench notification
```

### 运行特定测试函数

```bash
# 运行特定的测试函数
cargo bench --bench health_checker health_result_creation
cargo bench --bench config_processing config_serialization
cargo bench --bench notification template_rendering
```

## 查看结果

基准测试结果会显示在终端中，包括：

- 平均执行时间
- 标准差
- 最小/最大执行时间
- 性能变化趋势

结果也会保存到 `target/criterion/` 目录中，可以生成HTML报告：

```bash
# 生成HTML报告（需要安装 criterion-reporter）
cargo install criterion-reporter
cargo bench --bench health_checker -- --output-format=html
```

## 性能基准

以下是一些性能基准参考（在标准硬件上）：

### 健康检测器
- 健康结果创建: < 1μs
- 服务配置创建: < 1μs
- JSON序列化: < 10μs
- JSON反序列化: < 20μs

### 配置处理
- 配置创建: < 5μs
- TOML序列化: < 50μs
- TOML反序列化: < 100μs
- 配置验证: < 10μs

### 通知处理
- 简单模板渲染: < 10μs
- 复杂模板渲染: < 50μs
- 模板创建: < 100μs

## 注意事项

1. **环境一致性**: 基准测试结果会受到硬件、操作系统和系统负载的影响
2. **预热**: 第一次运行可能较慢，建议运行多次以获得稳定结果
3. **统计显著性**: 关注标准差，确保结果具有统计显著性
4. **回归检测**: 定期运行基准测试以检测性能回归

## 添加新的基准测试

要添加新的基准测试：

1. 在 `benches/` 目录下创建新的 `.rs` 文件
2. 使用 `criterion` crate 编写测试
3. 在 `Cargo.toml` 中添加 `[[bench]]` 配置
4. 更新此文档

示例：

```rust
use criterion::{black_box, criterion_group, criterion_main, Criterion};

fn my_benchmark(c: &mut Criterion) {
    c.bench_function("my_function", |b| {
        b.iter(|| {
            // 要测试的代码
            black_box(my_function())
        });
    });
}

criterion_group!(benches, my_benchmark);
criterion_main!(benches);
```