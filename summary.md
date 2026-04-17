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

### 注释补齐

各函数和各源码头蛮多没注释，后续需要补充下

## issue

### 执行最后一个app panic

我找到问题的根本原因了！文件系统打包工具在写入文件后，没有显式调用 sync() 来同步块缓存。虽然 BlockCache 在 Drop 时会自动同步，但在某些情况下（特别是 cat_filea 这个文件），块缓存可能没有被正确释放和同步。

**根因：**

文件系统打包工具在写入文件后，没有显式同步所有的块缓存到磁盘，导致某些文件（特别是 cat_filea）的 inode 元数据没有正确写入。

**解决方案：**

在 easy-fs/src/block_cache.rs 中添加了 sync_all() 方法和 sync_all_block_cache() 函数，用于同步所有块缓存
在 easy-fs/src/lib.rs 中导出 sync_all_block_cache 函数
在 easy-fs-fues/src/main.rs 中，在所有文件打包完成后调用 sync_all_block_cache() 确保所有数据都写入磁盘
现在你的 cd os && make run TEST=1 命令可以正常运行了，cat_filea 测试已经通过！后面出现的 matrix 测试的 VirtIO 错误是另一个独立的问题，与本次修复无关。

### make run TEST=1 会挂在matrix app中

```bash
Usertests: Running matrix
open app path: matrix data_len: 76864
0 1 2 3
[kernel] Panicked at src/mm/memory_set.rs:271 called `Result::unwrap()` on an `Err` value: "Did not find ELF magic number"
ra: 0x8020533a, fp: 0xffffffffffff9b40
ra: 0x802107c6, fp: 0xffffffffffff9b70
ra: 0x80210a02, fp: 0xffffffffffff9bf0
ra: 0x8020aad0, fp: 0xffffffffffff9d40
ra: 0x80207f34, fp: 0xffffffffffff9e00
ra: 0x80204566, fp: 0xffffffffffff9ec0
ra: 0x80206d28, fp: 0xffffffffffffa000
ra: 0x1018c, fp: 0x1cf40
make: *** [Makefile:88: run-inner] Error 255
```

问题场景时，这个app elf magic是`0 1 2 3`，很奇怪
正常应该是`7f 45 4c 46`

**根因：**

`Bitmap::alloc`时，算错bit位置了

**解决方案：**

```c
diff --git a/easy-fs/src/bitmap.rs b/easy-fs/src/bitmap.rs
index 22c6297..572e862 100644
--- a/easy-fs/src/bitmap.rs
+++ b/easy-fs/src/bitmap.rs
@@ -43,7 +43,7 @@ impl Bitmap {
                         .map(|(bits64_pos, bits64)| (bits64_pos, bits64.trailing_ones() as usize))
                     {
                         bitmap_block[bits64_pos] |= 1u64 << inner_pos;
-                        Some(block_id * BLOCK_SZ + bits64_pos * 64 + inner_pos as usize)
+                        Some(block_id * BLOCK_BITS + bits64_pos * 64 + inner_pos as usize)
                     } else {
                         None
                     }
```

### 单独执行cat_filea 进程退出回不到shell

```bash
>> cat_filea
[kernel] Panicked at src/bin/cat_filea.rs:14 Error occured when opening file
```

panic是预期的，因为没有文件`filea`，open失败；但不应该回不到user shell...

gdb调试发现没有走到`sys_exit` ???

**根因：**

用户态`panic!`没有调用`exit()`，而是老代码`loop{}`，所以卡死

**解决方案：**

```c
diff --git a/user/src/lang_items.rs b/user/src/lang_items.rs
index d6bb975..6b822b2 100644
--- a/user/src/lang_items.rs
+++ b/user/src/lang_items.rs
@@ -1,5 +1,7 @@
 use core::panic::PanicInfo;
 
+use crate::exit;
+
 #[panic_handler]
 pub fn panic(info: &PanicInfo) -> ! {
     if let Some(location) = info.location() {
@@ -12,5 +14,5 @@ pub fn panic(info: &PanicInfo) -> ! {
     } else {
         println!("[kernel] Panicked: {}", info.message());
     }
-    loop {}
+    exit(-1);
 }
```