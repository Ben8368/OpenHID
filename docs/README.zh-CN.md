<p align="center"><img src="../design/icon/openhid.png" width="128" alt="OpenHID" /></p>
<h1 align="center">OpenHID</h1>
<p align="center">使用 Rust 构建的原生、本地优先设备管理工具。</p>

[English](../README.md) · [开发指南](DEVELOPMENT.md) · [使用说明](USAGE.md) · [源码来源](PROVENANCE.md)

## 源码来源与独立维护

OpenHID 基于 [AprilNEA/OpenLogi](https://github.com/AprilNEA/OpenLogi) 开发。
记录的上游基线为
[`abc815acf9ebb4df06ccc574016bad2c9b4152ee`](https://github.com/AprilNEA/OpenLogi/commit/abc815acf9ebb4df06ccc574016bad2c9b4152ee)，
本次导入所用的本地分支快照为 `11db0bbebe10d761c0830ded6cc9cee39f38e160`。

新仓库仅从一次源码导入开始记录 Git 历史，不保留之前的提交、分支与标签。
此次导入包含已有的本地修改、OpenHID 更名、新品牌图标和来源声明，
并非未经修改的上游快照。早期开发历史可在原项目查看；导入源码不表示
我们拥有继承代码的原创署名。

后续由 [Ben8368](https://github.com/Ben8368) 及 OpenHID 贡献者独立维护，
拥有自己的开发方向、问题跟踪和后续版本。不自动同步上游；需要引入的
上游改动逐项评估。保留原作者及贡献者署名，以及原有
[MIT](../LICENSE-MIT) / [Apache-2.0](../LICENSE-APACHE) 双许可证。
完整说明见 [NOTICE](../NOTICE) 和 [源码来源文档](PROVENANCE.md)。

## 当前支持范围

继承的功能主要面向罗技 HID++ 鼠标、键盘，以及 UVC 摄像头，包括受支持
设备的改键、DPI、SmartShift、手势、按应用配置、电量和摄像头控制。
具体能力依设备型号、连接方式和系统而异。

OpenHID 名称代表后续方向，不表示已经支持所有 HID 设备。
狼蛛 AULA F87 PRO 等其他品牌仍需要单独适配设备协议。

后台 agent 负责 HID 和输入访问；桌面应用及动作环通过本地 IPC 与其通信。
配置以 TOML 保存在本机，无需账号。

## 从源码运行

安装稳定版 Rust 和[开发指南](DEVELOPMENT.md)列出的系统依赖后执行：

```sh
git clone https://github.com/Ben8368/OpenHID.git
cd OpenHID
cargo build -p openhid
cargo run -p openhid -- list
```

使用设备管理界面时，在一个终端运行 agent，在另一个终端运行桌面程序：

```sh
cargo run -p openhid-agent --bin openhid-agent
cargo run -p openhid-desktop
```

macOS 的桌面开发启动器会启动配套 agent。没有硬件时，可用
`openhid-agent-mock` 替代真实 agent，具体步骤见开发指南。
OpenHID 使用独立的配置和 IPC 名称，不自动迁移 OpenLogi 配置或系统授权。

## 发布与许可证

本次导入不发布安装包，也未配置 OpenHID 更新源；请先从源码构建。
上游的安装包、Homebrew 软件包、更新、签名和翻译服务不属于 OpenHID。
本仓库的 workspace crate 暂不发布至 crates.io。

OpenHID 使用重新绘制的独立图标，原 OpenLogi 专有品牌素材已替换。
设备图片仍可能从外部镜像下载，其权利和可用性由相应提供者决定。
源码保留 MIT / Apache-2.0 双许可证；HID++ 分支保留 0BSD 许可证。
本项目与 AprilNEA、罗技及狼蛛不存在官方关联或背书关系。
