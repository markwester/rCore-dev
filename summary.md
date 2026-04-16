## rust

`#[repr(C)]`指定结构体在内存中的布局与C一致


## os design

easyfs 文件系统的整体架构自下而上可分为五层：

磁盘块设备接口层：读写磁盘块设备的trait接口
块缓存层：位于内存的磁盘块数据缓存
磁盘数据结构层：表示磁盘文件系统的数据结构
磁盘块管理器层：实现对磁盘文件系统的管理
索引节点层：实现文件创建/文件打开/文件读写等操作

扁平化：仅存在根目录 / 一个目录，剩下所有的文件都放在根目录内。在索引一个文件的时候，我们直接使用文件的文件名而不是它含有 / 的绝对路径。
权限控制：我们不设置用户和用户组概念，全程只有单用户。同时根目录和其他文件也都没有权限控制位，即完全不限制文件的访问方式，不会区分文件是否可执行。
不记录文件访问/修改的任何时间戳。
不支持软硬链接。


SuperBlock 是一个磁盘上数据结构，它就存放在磁盘上编号为 0 的块的起始处。

## 问题

### Inode

```rust
pub struct Inode {
    block_id: usize,
    block_offset: usize,
    fs: Arc<Mutex<EasyFileSystem>>,
    block_device: Arc<dyn BlockDevice>,
}
```

这玩意儿是怎么表示一个文件的，只有一个block_id和block_offset

pub struct OSInodeInner {
    offset: usize,
    inode: Arc<Inode>,
}

## 开发计划

### 栈溢出检测

make run MODE=debug 时遇到内核启动init之后就无反应现象，定位出来是Debug模式下的内核代码体积过大，导致栈溢出

于是需要加一个栈溢出检测