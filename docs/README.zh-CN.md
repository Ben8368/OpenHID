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
受支持设备还会显示低电量提示与锁定键指示。具体能力依设备型号、连接方式
和系统而异。

后台 agent 负责 HID 和输入访问；桌面应用及动作环通过本地 IPC 与其通信。
配置以 TOML 保存在本机，无需账号。

OpenHID 名称代表后续方向，不表示已经支持所有 HID 设备。其他品牌需单独
适配协议。狼蛛 AULA F87 PRO 的无线灯效与睡眠控制现已支持；已验证的连接
方式、备份边界与剩余限制见[设备说明](devices/aula-f87-pro.md)。

## 从源码运行

安装稳定版 Rust 和[开发指南](DEVELOPMENT.md)列出的系统依赖后执行：

```sh
git clone https://github.com/Ben8368/OpenHID.git
cd OpenHID
cargo build -p openhid
cargo run -p openhid -- list
cargo run -p openhid-desktop
```

使用设备管理界面时，GUI 需要正在运行的 agent。源码开发会话可在另一个
终端启动：

```sh
cargo run -p openhid-agent --bin openhid-agent
```

macOS 的桌面开发启动器会启动配套 agent。没有硬件时，可用
`openhid-agent-mock` 替代真实 agent，具体步骤见开发指南。
OpenHID 使用独立的配置和 IPC 名称，不自动迁移 OpenLogi 配置或系统授权。

## 发布状态与许可证

优先[从源码构建](#从源码运行)。本仓库会打 GitHub Release 标签；带
`openhid-…` 资源名的 Linux 包用 [INSTALL-linux.md](INSTALL-linux.md) 中的
校验安装脚本安装（没有匹配资源包时可用 `--from-source` 安装本地
`target/release`）。若某个 Release 只有继承的 `openlogi-…` 文件名，则不是
OpenHID 资源包，请从源码构建。

内置更新器没有默认的 OpenHID 清单 URL。上游 OpenLogi 的安装包、Homebrew、
更新源、签名与翻译服务不属于 OpenHID。本仓库的 workspace crate 不发布到
crates.io。

OpenHID 使用重新绘制的独立图标，原 OpenLogi 专有品牌素材已替换。
设备图片仍可能从外部镜像下载，其权利和可用性由相应提供者决定。
源码保留 MIT / Apache-2.0 双许可证；HID++ 分支保留 0BSD 许可证。
本项目与 AprilNEA、罗技及狼蛛不存在官方关联或背书关系。
