# 下一角色包实现约束（advisor只读建议，尚未实现）

当前normal_pool workspace的SimCodec/HelperCompare固定四页，Filled只覆盖OwnerIO；Sim随后Store::create/open、Reference和Compare内部构造仍在池外。不能只增额度签收。

接口链：父PrepareRole(本地版本profile)→spawn→Bind(pid,instance)；子认证授权→RoleWorkspace::allocate(profile)→Filled(全部主要背板manifest)→Store::open/create_into_workspace或Reference::from_workspace运行；父真实Child::wait→Revoke→补回原精确页集合→ACK。Reference/Compare不给worker权限，Sim+worker5MiB共同≤64MiB。

包预建根/准备/候选/codec/IO/空间resident。空闲槽也实际初始化并保留，头部/页尾/解析暂存分别收费，只允许真实容量与页取整差作为padding，不能用grant减旧heap峰。Prepared阶段不得运行实际业务或worker；失败/丢回复/死亡不得早归还，补页部分失败继续fail-closed。

Store须真正接收并使用已有根，不能接收后又Engine::new；genesis/Opening写入已有背板。空间加载使用获准resident，地形导入128MiB峰独立，不由resident自动覆盖。Reference/普通Compare消费预建包。legacy与read_file/serde控制对象暂态须明确固定暂存或仍EXTRA；仅大对象有界不等于整个普通工作区闭合。

这是代码接入约束，不是实现、分配或性能证据。下一阶段把整包和真实broker接入作为完整可验证交付，不先发布名义大额grant。
