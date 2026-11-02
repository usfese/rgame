# rgame

这是一个rust游戏，为未来的magatroid通用游戏引擎作基底。

## TODO

### ECS

- [x] Entity系统
- [x] Component系统
- [x] System查询
- [ ] System运行
- [ ] System并行运行
- [ ] System自动拓扑排序

### 渲染器

- [ ] 渲染基本窗口
- [ ] forward+模块化可插拔渲染器架构
- [ ] 帧图
- [ ] 无绑定纹理
- [ ] 管线布局生成和管线缓存
- [ ] GPU异步计算着色器接口
- [ ] 网格着色器
- [ ] 集群光照
- [ ] 稀疏资源支持的立方体纹理阴影
- [ ] 可变速率着色
- [ ] 体积雾
- [ ] MSAA和TAA模块实现
- [ ] 光追管线
- [ ] 全局光照
- [ ] 光追反射

### 底层技术支持

- [ ] 基础IO
- [ ] 资源定义
- [ ] 异步加载资源和并行写入命令缓冲区

## 参考资源

- [Mastering-Graphics-Programming-with-Vulkan](https://github.com/PacktPublishing/Mastering-Graphics-Programming-with-Vulkan)
- Gregory, Jason. *Game Engine Architecture (2nd Edition)*. A K Peters/CRC Press, 2014. [ISBN: 9781466560017](https://books.google.com/books?id=MCQbBAAAQBAJ).
- Graham Sellers, with contributions from John Kessenich. *Vulkan Programming Guide: The Official Guide to Learning Vulkan*. Addison-Wesley Professional, 2016. [ISBN: 9780134464688](https://www.informit.com/store/vulkan-programming-guide-the-official-guide-to-learning-9780134464688).
- Graham Sellers, Richard S. Wright Jr., and Nicholas Haemel. *OpenGL SuperBible: Comprehensive Tutorial and Reference (7th Edition)*. Addison-Wesley Professional, 2015. [ISBN: 9780672337475](https://www.informit.com/store/opengl-superbible-comprehensive-tutorial-and-reference-9780672337475).
