# 参数 a0 存放内存中的值的所在地址
# 参数 a1 存放 expected
# 参数 a2 存放 new
# 返回值 a0 略有不同：这里若比较结果相同则返回 0 ，否则返回 1
# 而不是返回 CAS 之前内存中的值
cas:
    lr.w t0, (a0) # LR 将值加载到 t0
    bne t0, a1, fail # 如果值和 a1 中的 expected 不同，跳转到 fail
    sc.w t0, a2, (a0) # SC 尝试将值修改为 a2 中的 new
    bnez t0, cas # 如果 SC 的目标寄存器 t0 不为 0 ，说明 LR/SC 中间值被修改，重试
    li a0, 0 # 成功，返回值为 0
    ret # 返回
fail:
    li a0, 1 # 失败，返回值为 1
    ret # 返回