# 测量依赖

使用[peak_alloc 0.3.0](https://docs.rs/peak_alloc/0.3.0/)，MIT，无传递依赖，Cargo.lock checksum绑定原crate，原包保留以核源。已逐行查看本地registry源：realloc会新分配并复制旧数据，计数包含短暂共存；它会改变测量程序成本，不能声称无侵入或把新结果当原生System allocator性能。所有新同源驱动与参考均使用同一实际编译二进制，不扣仪表成本。其终身峰仅Rust requested分配，allocator rounding/元数据、栈、mapped、foreign和其他进程仍需独立计量。

也只读调查了tracking-allocator，但未引入；Cargo.lock没有该依赖。包调查日志只记录选择过程，不是额外实现或性能证据。
