# Core Rules（小册基础/进阶篇）

> 来源：《Rust 语言从入门到实战》开篇、01–12、18–20、29–30、答疑（一）（二）。
> Agent：按 ID 引用；写代码时违反则改正。

## Ownership

### `own-one-owner`
1. **规则**：任何时刻保证每个值只有一个所有者，作用域结束即释放。  
2. **为何**：所有权是内存安全根规则，违反会导致 double-free / use-after-free。  
3. **正例**：`let s2 = s1;` 后只用 `s2`。**反例**：move 后再用 `s1`。  
4. **来源**：02

### `own-move-default`
1. **规则**：非 Copy 类型赋值/传参默认 move，禁止默认“共享引用式复制”。  
2. **为何**：单所有权消除隐式共享带来的别名写风险。  
3. **正例**：函数吃掉 `String` 后不再用原变量。**反例**：假定像 Java 一样赋值后双方仍可用。  
4. **来源**：02、答疑一/02

### `own-copy-only-primitives`
1. **规则**：仅基础固定尺寸类型（整数/bool/float/char、由其组成的 tuple/array、不可变引用）默认 Copy；自定义类型默认 move。  
2. **为何**：显式优于隐式；`=` 复制无足迹，易藏性能与逻辑坑。  
3. **正例**：`let b = a_u32;` 两边可用。**反例**：以为 `struct Point {x:i64,y:i64}` 默认可 Copy。  
4. **来源**：02、11、答疑一/02

### `own-prefer-clone-over-copy-derive`
1. **规则**：需要多份所有权时优先显式 `.clone()`；仅当确需隐式按位复制且字段全 Copy 时才 `#[derive(Copy, Clone)]`。  
2. **为何**：`clone()` 留下可搜索足迹；Copy 是隐式行为，Rust 故意抬高其成本。  
3. **正例**：`foo(s.clone())`。**反例**：含 `Vec`/`String` 的类型强行 derive Copy。  
4. **来源**：02、11

### `own-mut-explicit`
1. **规则**：默认不可变；仅在后续会修改时加 `mut`，不用则去掉。  
2. **为何**：可变性足迹降低远程隐蔽修改导致的难查 bug。  
3. **正例**：`let mut v = Vec::new(); v.push(1);`。**反例**：全程只读却标 `mut`。  
4. **来源**：02、开篇词

### `own-shadow-ok`
1. **规则**：可用同名 `let` shadowing 转换类型/值，避免无意义新名字。  
2. **为何**：减少命名噪音，且类型可变。  
3. **正例**：`let s = foo(s);`。**反例**：为 shadow 伪造不必要中间名。  
4. **来源**：02

### `own-return-ownership-when-needed`
1. **规则**：若函数吃掉所有权且调用方还要用，要么返回所有权，要么改成借用参数。  
2. **为何**：避免无用的 move 进函数导致资源在帧结束被释放。  
3. **正例**：`fn f(s: String) -> String` 或 `fn f(s: &String)`。**反例**：`f(s); println!("{s}");`。  
4. **来源**：02、03

### `own-raii`
1. **规则**：堆资源随所有者离开作用域自动释放，禁止依赖手动 free 心智。  
2. **为何**：RAII 是所有权体系的落地机制。  
3. **正例**：局部 `String` 出花括号即 drop。**反例**：假设像 C 一样可忘记释放仍安全。  
4. **来源**：02、开篇词

### `own-stack-vs-heap-sizing`
1. **规则**：编译期可知尺寸的类型优先栈；动态尺寸走堆+指针管理。  
2. **为何**：固定尺寸才能做更强编译期检查与更优分配。  
3. **正例**：buffer 用 `[u8; N]`；可变长用 `Vec`。**反例**：巨大固定数组无脑上栈。  
4. **来源**：01、02


## Borrowing

### `borrow-alias-xor-mutate`
1. **规则**：同一时刻：多个共享借用 **或** 一个可变借用，二者不可交叠；可变借用互斥。  
2. **为何**：消灭数据竞争与别名写读不一致。  
3. **正例**：`&` 用完再 `&mut`。**反例**：`&mut` 存活期间再 `&` 或第二个 `&mut`。  
4. **来源**：03、答疑一/03

### `borrow-scope-nll`
1. **规则**：按引用最后一次使用结束借用作用域，而非花括号；据此安排读写顺序。  
2. **为何**：借用检查按活跃区间判定，顺序颠倒即编译失败。  
3. **正例**：用完 `&mut` 后再不可变读。**反例**：先创建 `&` 再 `&mut` 且后续仍用 `&`。  
4. **来源**：03

### `borrow-no-write-while-borrowed`
1. **规则**：存在任何借用时，禁止通过所有者直接赋值修改。  
2. **为何**：防止借用方读到被改写数据。  
3. **正例**：借用结束后再 `a = 20`。**反例**：`let r=&a; a=20; use(r)`。  
4. **来源**：03、答疑一/03

### `borrow-mut-is-exclusive-move`
1. **规则**：`&mut T` 再赋值按 move，不可 Copy；可变引用是“独家代理”。  
2. **为何**：若 Copy `&mut` 会破坏唯一可变借用。  
3. **正例**：`let r2 = r1;` 后只用 `r2`。**反例**：期望 `&mut` 像 `&` 一样复制。  
4. **来源**：03、答疑一/03

### `borrow-prefer-ref-params`
1. **规则**：函数只读用 `&T`/`&str`；需改用 `&mut T`；仅转移所有权时才吃 `T`。  
2. **为何**：参数签名即意图声明，减少所有权来回传递样板。  
3. **正例**：`fn foo(s: &str)`。**反例**：仅为读取而 `fn foo(s: String)`。  
4. **来源**：03、04

### `borrow-multilevel-deref-all-mut`
1. **规则**：经多层引用改目标时，路径上须全是可变引用并正确多级解引用。  
2. **为何**：中间夹 `&` 会挡住写权限。  
3. **正例**：`**c = 30`（`c: &mut &mut T`）。**反例**：路径含 `&` 却赋值。  
4. **来源**：03

### `borrow-auto-deref-methods`
1. **规则**：方法调用允许自动多级解引用；实现方法时优先 `&self`/`&mut self`。  
2. **为何**：调用侧更符合直觉；传 `self` 会消耗实例。  
3. **正例**：`(&&&&rect).area()`。**反例**：只读方法写成 `fn area(self)`。  
4. **来源**：05


## String

### `str-owned-vs-borrowed`
1. **规则**：拥有并可能修改用 `String`；只读视图用 `&str`；字面量是 `&'static str`。  
2. **为何**：所有权与切片视图分离，避免无必要堆分配与语义混淆。  
3. **正例**：API 入参 `&str`，需要拥有再 `.to_string()`。**反例**：到处存 `&String`。  
4. **来源**：04

### `str-api-take-str-not-string-ref`
1. **规则**：函数字符串参数优先 `&str`（可同时接 `&String` 与 `&str`），勿写死 `&String`。  
2. **为何**：`Deref` 使 `&String → &str`；`&String` 拒绝字面量。  
3. **正例**：`fn foo(s: &str)`。**反例**：`fn foo(s: &String)` 再传 `"hi"`。  
4. **来源**：04、11

### `str-no-byte-index-chars`
1. **规则**：禁止按字节下标当“字符”访问；按字符用 `.chars()`；切片须落在 UTF-8 边界。  
2. **为何**：UTF-8 变长，错误切片会 panic。  
3. **正例**：`s.chars().collect::<Vec<_>>()`。**反例**：`s[0]` 或对中文 `&s[..5]`。  
4. **来源**：01、04、答疑一/04

### `str-utf8-checked-convert`
1. **规则**：`Vec<u8>`/`&[u8]` → 字符串用 `from_utf8`（返回 Result）；禁止默认 `*_unchecked`。  
2. **为何**：无效 UTF-8 是客观复杂性，必须显式处理。  
3. **正例**：`String::from_utf8(bytes)?`。**反例**：生产路径 `from_utf8_unchecked`。  
4. **来源**：04、29

### `str-domain-types`
1. **规则**：路径用 `Path`/`PathBuf`，OS 原生串用 `OsStr`/`OsString`，C 交互用 `CStr`/`CString`；勿用裸 `String` 糊弄。  
2. **为何**：场景类型携带跨平台/终止符等额外约束。  
3. **正例**：文件系统 API 用 `Path`。**反例**：路径一律 `String` 手搓分隔符。  
4. **来源**：04

### `str-parse-via-fromstr`
1. **规则**：字符串→类型走 `.parse::<T>()`/`FromStr`，并处理 Result；复杂格式用 serde。  
2. **为何**：统一社区接口，失败可自定义处理。  
3. **正例**：`"10".parse::<u32>()?`。**反例**：静默吞解析错误。  
4. **来源**：04

### `str-bytes-for-protocol`
1. **规则**：纯 ASCII/协议二进制用字节串 `b"..."` / `&[u8]`，勿强行 `String`。  
2. **为何**：系统/网络场景更紧凑准确。  
3. **正例**：`let hdr: &[u8] = b"GET ";`。**反例**：协议字段全用 UTF-8 `String`。  
4. **来源**：01、04

### `str-vec-slice-symmetry`
1. **规则**：`String`↔`&str` 对称于 `Vec<T>`↔`&[T]`；集合 API 入参优先切片 `&[T]`/`&str`。  
2. **为何**：同一 Deref 模式提升 API 通用性。  
3. **正例**：`fn foo(xs: &[u32])` 可接 `&Vec`。**反例**：只接 `&Vec<u32>`。  
4. **来源**：04


## Struct / Enum

### `struct-own-fields-for-business`
1. **规则**：业务模型结构体字段默认持有所有权（`String`/`Vec` 等）；引用字段留给性能优化阶段并标注生命周期。  
2. **为何**：自包含类型避免生命周期传染；初学/业务优先实用。  
3. **正例**：`struct User { name: String }`。**反例**：业务层一上来全是 `&str` 字段。  
4. **来源**：05、20

### `struct-partial-move-aware`
1. **规则**：移动结构体字段后整体不可用；只需字段引用时用 `ref`/`ref mut` 或 `&field`。  
2. **为何**：部分移动后打印/再借整体会失败。  
3. **正例**：`let User { ref name, .. } = u;`。**反例**：`let email = u.email;` 后再 `println!("{:?}", u)`。  
4. **来源**：05、06

### `struct-update-syntax`
1. **规则**：少量字段变更用 `..old`；结构体需 `mut` 才能改字段。  
2. **为何**：减少样板，字段更新意图清晰。  
3. **正例**：`User { email: new, ..old }`。**反例**：不可变实例上改字段。  
4. **来源**：05

### `struct-newtype-for-semantics`
1. **规则**：需新语义/实现 orphan 规则绕过时用单字段元组结构体 newtype。  
2. **为何**：新类型可挂 impl/From，并屏蔽内部细节。  
3. **正例**：`struct MyError(String);`。**反例**：直接对外部类型 `impl From` 违反 orphan。  
4. **来源**：07、18、答疑一/05

### `struct-impl-methods-three-self`
1. **规则**：实例方法三态：`self` / `&self` / `&mut self`；无 self 的为关联函数；构造约定 `new`/`from_*`。  
2. **为何**：所有权意图显式，无隐藏 this。  
3. **正例**：`Rectangle::new(w,h)`。**反例**：只读计算却吃 `self`。  
4. **来源**：05

### `enum-payload-for-variants`
1. **规则**：变体可挂不同负载；用 enum 表达“同一时刻一种可能 + 附带数据”。  
2. **为何**：和类型承载分支与数据内聚。  
3. **正例**：`KeyPress(char)`、`Click{x,y}`。**反例**：用魔法数字+平行字段模拟状态。  
4. **来源**：06

### `enum-for-config-struct-for-model`
1. **规则**：领域主体用 struct；配置/错误/分支聚合用 enum；勿对变体单独 impl。  
2. **为何**：小册明确：新类型主要靠结构体，枚举辅助分类。  
3. **正例**：`Shape` enum + `Rectangle` struct。**反例**：`impl Foo::AAA {}`。  
4. **来源**：06

### `enum-match-exhaustive`
1. **规则**：`match` 必须穷尽；临时统一处理才用 `_`；各臂返回类型一致，异型用 enum/`Box<dyn Trait>` 包装。  
2. **为何**：穷尽匹配阻止漏分支；静态类型要求统一返回。  
3. **正例**：覆盖全部变体。**反例**：漏变体或臂返回不同类型裸值。  
4. **来源**：06、答疑一/06

### `enum-pattern-ref-to-avoid-move`
1. **规则**：模式匹配默认可能 move；只读字段加 `ref`，可变加 `ref mut`。  
2. **为何**：匹配是释放所有权的路径之一。  
3. **正例**：`Shape::Circle { ref origin, radius }`。**反例**：匹配出 `String` 后再用原结构体。  
4. **来源**：06

### `enum-closed-vs-trait-open`
1. **规则**：封闭已知类型集用 enum；库要可扩展用 trait（`impl Trait`/`dyn Trait`）。  
2. **为何**：enum 改动需改匹配；trait 开放实现。  
3. **正例**：库回调用 trait。**反例**：库公共 API 用不可扩展 enum 锁死。  
4. **来源**：10


## Generics

### `gen-explicit-casts`
1. **规则**：跨数字类型运算显式 `as`/`From`/`Into`，禁止依赖隐式转换。  
2. **为何**：显式留下足迹，避免 JS 式隐式策略。  
3. **正例**：`a * (b as f32)`。**反例**：`1.0f32 * 10` 指望自动通。  
4. **来源**：07

### `gen-monomorphize-with-bounds`
1. **规则**：泛型函数必须给够 trait bound（如 `Display`）；约束多时用 `where`。  
2. **为何**：无 bound 的 T 类型空间过大，无法调用方法。  
3. **正例**：`fn print<T: Display>(p: Point<T>)`。**反例**：无约束却 `println!("{}", t)`。  
4. **来源**：07、09

### `gen-turbofish-when-ambiguous`
1. **规则**：推导不清时用 `::<>` 喂类型信息。  
2. **为何**：编译期单态化需要具体类型。  
3. **正例**：`Point::<u32> { .. }`、`"10".parse::<u32>()`。**反例**：含糊调用导致推断失败却硬猜。  
4. **来源**：07、04

### `gen-type-alias-onion`
1. **规则**：深层嵌套用 `type` 别名；模块级可 `type Result<T> = Result<T, ThisError>`。  
2. **为何**：洋葱类型难读，别名控复杂度。  
3. **正例**：`type MyMap = HashMap<String, Vec<u8>>;`。**反例**：签名里嵌套五层尖括号。  
4. **来源**：07

### `gen-no-duplicate-methods-overlap`
1. **规则**：勿为“能覆盖同一具化类型”的泛型 impl 与具体 impl 定义同名方法。  
2. **为何**：调用歧义，编译失败。  
3. **正例**：泛型 T: Copy 的方法与 `String` 同名可共存。**反例**：无约束 `impl<T>` 与 `impl String` 同名方法。  
4. **来源**：答疑一/07


## Option / Result / Iterator

### `opt-none-not-sentinel`
1. **规则**：缺失用 `Option::None`，勿用空串/0/NULL 哨兵混表示“无”。  
2. **为何**：空值独立维度，消灭十亿美元错误。  
3. **正例**：`Option<String>` 区分 `None` 与 `Some("")`。**反例**：用 `""` 表示未设置。  
4. **来源**：08

### `opt-prefer-combinators`
1. **规则**：优先 `map`/`and_then`/`as_ref`/`take` 等组合子；避免无脑 `unwrap`。  
2. **为何**：不解包即可变换，保留上下文。  
3. **正例**：`opt.as_ref().map(|s| s.len())`。**反例**：层层 `unwrap` 再拼。  
4. **来源**：08

### `opt-unwrap-only-with-invariant`
1. **规则**：`unwrap`/`expect` 仅在不变量保证成功或测试中使用；生产用 `expect` 须唯一清晰消息。  
2. **为何**：二者会 panic；`expect` 便于深栈定位。  
3. **正例**：`env::var("X").expect("wrapper must set X")`。**反例**：用户输入路径 `.unwrap()`。  
4. **来源**：08、18

### `res-map-err-and-and-then`
1. **规则**：错误侧用 `map_err`；成功链用 `and_then`/`map`；Option↔Result 注意信息增减（`ok_or`/`ok`）。  
2. **为何**：链式处理比嵌套 match 更紧凑。  
3. **正例**：`open(...).map_err(...).and_then(|f| ...)`。**反例**：`result.ok()` 丢错误信息却仍当错误处理。  
4. **来源**：08、18

### `iter-three-modes`
1. **规则**：`iter()`→`&T`，`iter_mut()`→`&mut T`，`into_iter()`→`T`（消耗集合）；`for x in c` 默认 `into_iter`。  
2. **为何**：所有权三态贯彻到迭代。  
3. **正例**：只读 `for x in &v`；要所有权 `for x in v`。**反例**：还要用集合却 `for x in v`。  
4. **来源**：08

### `iter-prefer-over-index`
1. **规则**：遍历优先迭代器；禁止用下标从 `Vec<非Copy>` move 出元素。  
2. **为何**：下标不安全/受限；迭代器边界安全且可取所有权。  
3. **正例**：`for s in v { ... }`。**反例**：`let a = v[0];`（`String`）。  
4. **来源**：01、08

### `iter-hashmap-own-via-into-iter`
1. **规则**：取 `HashMap` 值所有权用 `into_iter`/`into_values` 等消耗型 API，而非索引 move。  
2. **为何**：与 Vec 同一权限模型。  
3. **正例**：`for (k,v) in map { ... }`。**反例**：指望 `map[k]` move 出值。  
4. **来源**：答疑一/08


## Trait design

### `trait-as-capability-bound`
1. **规则**：用 `T: TraitA + TraitB` 表达能力配置；约束与能力一体两面。  
2. **为何**：缩小类型空间并声明可调用接口。  
3. **正例**：`T: Display + PartialEq`。**反例**：无 bound 的万能 `T`。  
4. **来源**：09

### `trait-import-for-methods`
1. **规则**：调用 trait 方法必须把 trait 导入当前 scope。  
2. **为何**：惰性能力配置，避免方法全集污染。  
3. **正例**：`use crate::Shape; a.play();`。**反例**：只 use 类型不 use trait 却调方法。  
4. **来源**：09

### `trait-supertrait-not-inheritance`
1. **规则**：`trait Circle: Shape` 表示实现 Circle 必须同时实现 Shape；约束彼此平等，非 OOP 继承。  
2. **为何**：消除冗余 bound，不是子类化。  
3. **正例**：实现两者。**反例**：只 impl Circle。  
4. **来源**：09

### `trait-orphan-rule`
1. **规则**：`impl Trait for Type` 时 Trait 或 Type 至少一方在当前 crate；否则 newtype。  
2. **为何**：保证实现连贯性。  
3. **正例**：`impl From<io::Error> for MyError`。**反例**：`impl From<io::Error> for String`。  
4. **来源**：09、18

### `trait-assoc-type-vs-param`
1. **规则**：一对一时用关联类型（impl 时必须具化）；需对同一类型多套实现/延迟具化用泛型参数 trait。  
2. **为何**：类型参数可“多次实现”；关联类型更简洁但无多态重载。  
3. **正例**：`Add<Point>` 与 `Add<i32>` 两套。**反例**：仅关联类型重复 `impl Add for Point`。  
4. **来源**：10

### `trait-impl-trait-same-concrete`
1. **规则**：`impl Trait` 返回值在函数内必须是同一具体类型；多分支不同具体类型改用 `Box<dyn Trait>`。  
2. **为何**：`impl Trait` 是静态单一隐藏类型，非运行时多态。  
3. **正例**：单类型返回 `impl TraitA`。**反例**：if/else 返回 Atype/Btype 却写 `impl Trait`。  
4. **来源**：10

### `trait-dyn-vs-generic`
1. **规则**：要静态性能/单态化→泛型或 `impl Trait`；要异构集合/运行时分支→`Box<dyn Trait>`/`&dyn Trait`；注意对象安全。  
2. **为何**：dyn 有虚调用与体积优势；非对象安全不可 dyn。  
3. **正例**：`Vec<Box<dyn Trait>>`。**反例**：trait 含 `fn new()->Self` 还要 dyn。  
4. **来源**：10、12

### `trait-object-safety`
1. **规则**：面向 dyn 的 trait：方法用 `&self`/`&mut self`/`Box<Self>`；禁止返回 Self、按值 self、泛型方法、关联常量、无接收者关联函数。  
2. **为何**：对象安全限制 vtable 可表达性。  
3. **正例**：`fn foo(&self)`。**反例**：在 trait 里放构造器 `new() -> Self`。  
4. **来源**：10

### `trait-ufcs-for-conflicts`
1. **规则**：同名方法冲突时用 `<Type as Trait>::method(&x)`。  
2. **为何**：完全限定路径消除歧义，无覆盖语义。  
3. **正例**：`<A as Shape>::play(&a)`。**反例**：假设类型方法覆盖 trait 方法后 trait 版消失。  
4. **来源**：09


## Std traits

### `std-derive-debug-default-clone`
1. **规则**：调试 derive `Debug`；合理默认值 derive `Default`；深拷贝 derive `Clone`；字段均满足方可 derive。  
2. **为何**：样板自动生成；字段未实现则整体不能 derive。  
3. **正例**：`#[derive(Debug, Default, Clone)]`。**反例**：含未 Debug 字段却 derive Debug。  
4. **来源**：05、11

### `std-display-manual-tostring-blanket`
1. **规则**：用户可见格式手动 `impl Display`；不要手写 `ToString`（有 Display 即有 ToString）。  
2. **为何**：标准库 blanket：`T: Display => ToString`。  
3. **正例**：`impl fmt::Display for Point`。**反例**：只 impl ToString。  
4. **来源**：11

### `std-eq-ord-float-careful`
1. **规则**：相等/排序：`PartialEq`/`PartialOrd`；总序容器 key 需 `Eq+Ord`；浮点只有 Partial*，勿当 BTree key。  
2. **为何**：NaN 破坏自反性。  
3. **正例**：`#[derive(PartialEq, Eq, PartialOrd, Ord)]` 用于整数结构体。**反例**：`BTreeSet<f64>`。  
4. **来源**：11

### `std-copy-requires-all-copy-fields`
1. **规则**：仅当所有字段 Copy（无堆所有权字段）才 derive Copy；且必须同时 Clone。  
2. **为何**：Copy 浅拷贝会与单所有权冲突。  
3. **正例**：`Point {x:u32,y:u32}`。**反例**：含 `Vec` 的结构体 Copy。  
4. **来源**：11

### `std-clone-is-ok`
1. **规则**：先用 `.clone()`/`to_owned()` 打通正确性，再优化；引用变所有权优先 Clone/ToOwned。  
2. **为何**：小册明确：不要怕 clone；性能优化是第三阶段。  
3. **正例**：`let s: String = slice.to_owned();`。**反例**：过早 unsafe/生命周期炫技。  
4. **来源**：11、20

### `std-from-impl-into-free`
1. **规则**：类型转换实现 `From<T>`，获得免费 `Into`；`?` 依赖 `From` 做错误转换。  
2. **为何**：社区惯例与 `?` 机制。  
3. **正例**：`impl From<io::Error> for MyError`。**反例**：只 impl Into 或两边都外部类型。  
4. **来源**：11、18

### `std-deref-not-inheritance`
1. **规则**：用 Deref 做智能指针/字符串切片转换；禁止用 Deref 模拟 OOP 继承。  
2. **为何**：Deref 不完整且违反意图。  
3. **正例**：`&String`→`&str`。**反例**：父子类型靠 Deref“继承”方法。  
4. **来源**：11、04

### `std-drop-for-foreign-resources`
1. **规则**：仅在管理外部资源（如 C 分配）时手写 Drop；普通 Rust 资源靠自动 drop。  
2. **为何**：Drop 是自定义清理钩子。  
3. **正例**：FFI 包装类型 Drop 调 C free。**反例**：仅为打印乱 impl Drop。  
4. **来源**：11、29–30

### `std-closure-fn-hierarchy`
1. **规则**：按捕获选择：`Fn` ⊂ `FnMut` ⊂ `FnOnce`；吃环境所有权→FnOnce 只能调一次；无捕获可降为 `fn` 指针。  
2. **为何**：闭包类型由捕获行为决定。  
3. **正例**：只读捕获用 `Fn`。**反例**：`FnOnce` 闭包调用两次。  
4. **来源**：11、01


## Smart pointers

### `sp-box-heap-own`
1. **规则**：需要堆分配且独占所有权、或返回/存储 `dyn Trait` 时用 `Box<T>`。  
2. **为何**：盒化保证堆上且持有所有权，规避悬垂引用。  
3. **正例**：`fn f() -> Box<dyn Trait>`。**反例**：返回局部 `&T`。  
4. **来源**：10、12

### `sp-box-deref-moves-if-not-copy`
1. **规则**：对非 Copy 的 `Box<T>` 解引用 `*boxed` 会移出所有权，之后不可再用 Box。  
2. **为何**：解引用是 `Box::new` 的逆操作。  
3. **正例**：Copy 类型解引用后 Box 仍可用。**反例**：`let p = *boxed_point; use(boxed)`。  
4. **来源**：12

### `sp-arc-shared-own-no-direct-mut`
1. **规则**：跨所有者共享用 `Arc`（多线程）/`Rc`（单线程）；`clone` 只增计数；修改内容需 `Mutex`/`RwLock`/`RefCell` 等。  
2. **为何**：共享所有权下直接可变会破坏其他持有者。  
3. **正例**：`Arc<Mutex<T>>`。**反例**：`Arc<T>` 上调 `&mut self` 方法。  
4. **来源**：12、答疑二/12

### `sp-prefer-box-dyn-in-structs`
1. **规则**：结构体存 trait 对象优先 `Box<dyn Trait>`/`Arc<dyn Trait>`，避免裸 `&dyn Trait`（生命周期标注）。  
2. **为何**：拥有型指针自包含，少传染 `'a`。  
3. **正例**：`struct S { x: Box<dyn Trait> }`。**反例**：`x: &dyn Trait` 无寿命。  
4. **来源**：12、20

### `sp-arc-vs-ref`
1. **规则**：借用检查过痛或需独立生命周期时升为 `Arc/Rc` 共享所有权；短生命周期只读优先 `&`。  
2. **为何**：所有权自管理比依赖外部 scope 简单。  
3. **正例**：多任务共享配置 `Arc`。**反例**：处处 Arc 替代本可简单的借用。  
4. **来源**：12


## Error handling

### `err-classify-recoverable`
1. **规则**：不可恢复用 `panic!`/`todo!`/`unimplemented!`/`unreachable!`；可恢复一律 `Result`。  
2. **为何**：策略不同，边界由业务决定。  
3. **正例**：配置缺失可 `Result`；不变量破坏可 panic。**反例**：所有失败都 panic。  
4. **来源**：18

### `err-must-use-result`
1. **规则**：禁止忽略 `Result`；不处理须 `let _ = ...` 显式丢弃并知情。  
2. **为何**：`unused_must_use` 强制面对错误。  
3. **正例**：`foo()?;` 或 match。**反例**：`foo();` 丢掉 Result。  
4. **来源**：18

### `err-enum-thiserror-in-libs`
1. **规则**：库/模块错误用 enum + `thiserror`（`Error`/`Display`/`#[from]`）；实现 `std::error::Error`。  
2. **为何**：生态协议；减少样板；`#[from]` 服务 `?`。  
3. **正例**：`#[derive(Error, Debug)] enum E { #[from] Io(io::Error), ... }`。**反例**：库对外只返回 `String` 错误。  
4. **来源**：18、答疑二/18

### `err-anyhow-at-app-boundary`
1. **规则**：应用层/二进制统一 `anyhow::Result<T>` 接收多源错误；处理时 `downcast` 分派。  
2. **为何**：统一接收协议，减少每模块自定义 Result。  
3. **正例**：`fn main_logic() -> anyhow::Result<()>`。**反例**：应用层每个文件一个 Result 别名。  
4. **来源**：18、答疑二/18

### `err-question-mark-bubble`
1. **规则**：用 `?` 防御式早归；确保存在 `From<下层错误> for 返回错误`（或 map_err/thiserror from）。  
2. **为何**：`?` 隐式 From 转换，类型不齐会编译失败。  
3. **正例**：返回 `MyError` 且 `impl From<io::Error>`。**反例**：返回 `Result<_, String>` 却对 io::Error 直接 `?`。  
4. **来源**：18

### `err-layered-bubble`
1. **规则**：错误按依赖树冒泡；在合适架构层处理（可演进到 UI）；构造/传递/处理分层。  
2. **为何**：错误系统是层级工程，非单点。  
3. **正例**：底层 thiserror，边界 anyhow。**反例**：每层都 unwrap。  
4. **来源**：18


## Macros

### `macro-use-sparingly`
1. **规则**：宏用于去样板/派生能力/DSL；禁止滥用导致难调试。  
2. **为何**：宏在编译前变换，IDE/排错成本高。  
3. **正例**：`#[derive(Debug, Error)]`。**反例**：本可用函数的逻辑硬写声明宏。  
4. **来源**：19

### `macro-declarative-for-repetition`
1. **规则**：简单重复代码用 `macro_rules!`；导出加 `#[macro_export]`；优先精确 `use` 宏而非全局 `macro_use`。  
2. **为何**：声明宏匹配+生成；精确导入更清晰。  
3. **正例**：小项目工具宏。**反例**：遗留 `#[macro_use] extern crate` 无脑全导入。  
4. **来源**：19

### `macro-expand-to-verify`
1. **规则**：编写/怀疑宏时用 `cargo expand` 核对展开。  
2. **为何**：展开即真相。  
3. **正例**：改宏后 expand 对比。**反例**：凭猜测改匹配臂。  
4. **来源**：19

### `macro-lint-levels`
1. **规则**：原型可用 `allow`；正式代码用 `warn`/`deny`/`forbid` 收紧；关键安全用 `forbid(unsafe_code)` 等。  
2. **为何**：lint 属性控制质量门槛。  
3. **正例**：成熟 crate 去掉全局 allow。**反例**：长期 `#![allow(dead_code)]`。  
4. **来源**：19、答疑二/19


## Lifetimes

### `life-annotate-struct-borrows`
1. **规则**：结构体存引用必须加生命周期参数并标到每个引用字段；impl 同步带上。  
2. **为何**：告知借用检查器对外部资源的依赖。  
3. **正例**：`struct Url<'a> { host: &'a str }`。**反例**：`struct Url { host: &str }`。  
4. **来源**：20

### `life-avoid-exposing-in-api`
1. **规则**：公共 API 尽量不暴露生命周期（持有所有权或智能指针）；避免像 Cow 那样向上传染。  
2. **为何**：社区共识：生命周期噪音与传染伤害可用性。  
3. **正例**：对外 `String`/`PathBuf`。**反例**：公开 API 满天 `'a`。  
4. **来源**：20

### `life-fn-output-tied-to-inputs`
1. **规则**：多输入引用且返回引用时，显式标注返回值与哪些输入同寿；编译器不靠函数体逻辑猜。  
2. **为何**：签名级分析；取输入寿命交集。  
3. **正例**：`fn longest<'a>(x:&'a str,y:&'a str)->&'a str`。**反例**：返回局部引用。  
4. **来源**：20

### `life-method-defaults-to-self`
1. **规则**：方法返回引用默认与 `self` 绑定；若返回其他参数引用须显式标注。  
2. **为何**：省略规则以 Self 为默认。  
3. **正例**：`fn name(&self) -> &str { &self.name }`。**反例**：返回参数 `a` 却无标注。  
4. **来源**：20

### `life-static-for-immortal`
1. **规则**：`'static` 仅用于程序全程有效数据（字面量/静态项）；勿滥用延长局部。  
2. **为何**：`'static` 是最长寿命。  
3. **正例**：`let s: &'static str = "hi";`。**反例**：把临时 `String` 标成 `'static`。  
4. **来源**：20、04

### `life-optimize-later`
1. **规则**：先所有权跑通→再架构→最后才用引用+生命周期做分配优化。  
2. **为何**：小册三阶段；上层业务几乎不必写 `'a`。  
3. **正例**：解析先 `String` 字段，热点再改切片。**反例**：第一版就上零拷贝生命周期迷宫。  
4. **来源**：20

### `life-is-generic-param`
1. **规则**：把 `'a` 当与 `T` 同级的泛型：`T` 空间展开，`'a` 时间展开。  
2. **为何**：同一套 `<>` 机制给编译器喂信息。  
3. **正例**：`fn foo<'a, T>(...)`。**反例**：把 `'a` 理解成运行时标签可赋值。  
4. **来源**：20、答疑二/20


## Unsafe

### `unsafe-minimize-boundary`
1. **规则**：Unsafe 缩到最薄封装层；上层 100% Safe；`unsafe` 表示“编译器不担保，作者须担保”。  
2. **为何**：审计面从全量缩到极小层。  
3. **正例**：safe 函数内局部 `unsafe { ... }`。**反例**：大面积业务逻辑包在 unsafe 里。  
4. **来源**：29

### `unsafe-five-powers-only`
1. **规则**：仅在需要时使用五类能力：解引用裸指针、调 unsafe 函数、可变 static、unsafe trait、union 字段；并显式 `unsafe`。  
2. **为何**：足迹隔离信任边界。  
3. **正例**：解引用 `*const T` 包 unsafe。**反例**：以为 unsafe 块关闭所有检查（数组越界仍查）。  
4. **来源**：29

### `unsafe-safe-abstraction`
1. **规则**：对外暴露安全 API；内部 unsafe 必须证明无 UB（如切片不重叠）。  
2. **为何**：封装后调用方按 Safe 使用。  
3. **正例**：`split_at_mut` 式安全包装。**反例**：把裸 `*mut` 泄漏到公共 API。  
4. **来源**：29、30

### `unsafe-no-unchecked-by-default`
1. **规则**：`get_unchecked`/`from_utf8_unchecked` 仅在已证边界/编码且性能关键路径使用。  
2. **为何**：跳过检查换速度，代价是 UB 风险。  
3. **正例**：热循环前 assert 再 unchecked。**反例**：普通业务默认 unchecked。  
4. **来源**：04、29

### `unsafe-ffi-repr-c-and-wrap`
1. **规则**：FFI 结构体 `#[repr(C)]`；外部函数 `extern` + 调用处 unsafe；再包一层 safe Rust API；可用 bindgen。  
2. **为何**：ABI 兼容与边界信任隔离。  
3. **正例**：`fn cos(...) { unsafe { ccosf(...) } }`。**反例**：公共模块直接到处调 extern。  
4. **来源**：30

### `unsafe-avoid-mut-static`
1. **规则**：避免可变全局 static；非要用则读写均 `unsafe` 并文档化同步假设。  
2. **为何**：可变全局是糟糕模型，且有数据竞争风险。  
3. **正例**：用 `Atomic`/`Mutex` 替代。**反例**：随意 `static mut` 当全局状态。  
4. **来源**：29


## 跨切面原则（开篇 + 全书反复强调）

| ID | 规则 | 来源 |
|---|---|---|
| `meta-explicit-footprints` | 有成本的操作必须留足迹：`mut`、`clone`、`unsafe`、`as`、`unwrap`/`expect` | 02、11、29 |
| `meta-ownership-triaility` | 设计 API 时始终显式三态：拥有 / 共享借 / 独占借 | 03、05、08、11、12 |
| `meta-type-more-info` | 用更精确类型喂编译器（场景字符串、newtype、enum 错误） | 开篇、04、07、18 |
| `meta-practical-first` | 先正确再优雅再性能；业务层优先自包含所有权 | 20、11 |
| `meta-compiler-is-assistant` | 把编译错误当设计反馈，按提示改所有权/约束/寿命，而非绕过 | 开篇、全文 |
