<h1 align="center">Monty</h1>

<h1 align="center">A Strongly Typed Python Dialect</h1>

## Index

- [Index](#index)
- [Brief](#brief)
- [Concepts under development](#concepts-under-development)
  - ["automatic unions"](#automatic-unions)
  - ["Type narrowing"](#type-narrowing)
  - ["Deviated instance types"](#deviated-instance-types)
- [Related projects](#related-projects)
  - ["prior art"](#prior-art)

## Brief

Monty `(/ˈmɒntɪ/)` is an attempt to provide a completely organic alternative
dialect of Python equipped with a stronger, safer, and smarter type system.

At a high level monty can be closely compared with what TypeScript does for
JavaScript. The core contrast between Monty and TypeScript however is
that TS is a strict syntactical superset of JS, Monty is a strict syntactical
subset of Python; meaning that TS adds backwards incompatible syntax to JS
where Monty disallows existing Python syntax and semantics in a backwards
compatible manner.

Monty is intended to be compiled to native executable binaries or WASM via the
use of [cranelift] (and maybe [llvm] if support for that ever lands.)

## Concepts under development

Apart from the regular semantics of interpreter Python, Monty will disallow
parts of the language selectively (depending on how hard the feature is to
translate to compiled code.)

### "automatic unions"

It's useful to be able to represent multiple types of values within one object.
_cough cough_ polymorphism _cough cough_

Variables, in Monty, may only have one type per scope.
you may not re-assign a value to a variable with a different type.

```py
def badly_typed():
    this = 1
    this = "foo"
```

You may however have a union of types, which is represented like a tagged union
in C or an enum in Rust. `typing.Union[T, ...]` is the Pythonic way to annotate
a union explicitly but in Monty you may use the newer literal syntax `T | U` from
[PEP604]:

```py
def correctly_typed():
    this: int | str = 1
    this = "foo"
```

But wait! say you have the following code:

```py
def foo() -> int:
    return 1

def bar() -> str:
    return "foo"

def baz(control: bool):
    x = foo() if control else bar()
```

What's the type of `x` in the function `bar` now?

Some might expect this to be a type error after all `foo` and `bar` return
incompatible types and they try get associated with `x` but this isn't the case
unless you explicitly annotate `x` to be one of `int` or `str`.

What happens instead is that the compiler will "synthesize" (create) a union
type for you so the type of `x` will be:

 * `int | str` or
 * `Union[int, str]`.

### "Type narrowing"

Type narrowing [is not a new concept] and its been around for a while in typecheckers.

The idea is, roughly, that you can take a union type and dissasemble it into one of its
variants through a type guard like:

```py
x: int | str | list[str]


if isinstance(x, int):
    # x is now considered an integer in this branch of the if statement
elif isinstance(x, str):
    # x is now considered a string here.
else:
    # exhaustive-ness checks will allow `x` to be treated as a list of strings here.
```

### "Deviated instance types"

Inspired from [this section of the RPython documentation][rpython-instances] Derived Instance Types
work very similarly. For example take the following class:

```py
class Thing:
    attr1: int
    attr2: list[str]
```

The memory layout of class `Thing` we'll call "Layout 1" will contain an integer and a list and by default
that's all that was said about the class so that's all monty can do with it for now any other attribute access
either getting or setting will invoke a type error to be reported to the user.

Here's where the idea of "diverging" an "instance"s "type" comes in:

```py
THING = Thing()
THING.attr3 = "blah blah"
```

This value would constant at runtime but lazily initialized at compile time but that's besides the point.

`THING` is an instance of `Thing` meaning the layout of the constant is specified in the class definition.
but we then try and set an attribute on the instance and at first glance it looks like an error but it actually
lets us do very clever things with the way we model values and types with monty.

The memory layout of `THING` is now "Layout 1, 0" (read as the first diverged layout from 1) and in rough
C pseudo code will be structured something like:

```c
struct Thing_Layout_1 {
    integer_type attr1;
    list_str_type attr2; 
}

struct Thing_Layout_1_0 {
    Thing_Layout_1 head;
    string_type attr3;
}
```

If `THING` were not to be a constant and instead a static module-level variable then you
are also free to set and modify the value of `THING.attr3` ala

```py
thing = Thing()

def whatever(blah: str, n: int):
    thing.attr3 = blah * n
```

## Related projects

### ["prior art"](https://github.com/rust-lang/rfcs/blob/master/text/2333-prior-art.md)

- [Cython](https://github.com/cython/cython)
- [Numba](https://github.com/numba/numba)
- [Nuitka](https://github.com/Nuitka/Nuitka)
- [Peggen](https://github.com/gvanrossum/pegen)
- [MyPy](https://github.com/python/mypy)
- [PyPy](https://foss.heptapod.net/pypy/pypy)
- [RPython](https://foss.heptapod.net/pypy/pypy/-/tree/branch/default/rpython)

[cranelift]: https://github.com/bytecodealliance/wasmtime/tree/main/cranelift
[llvm]: https://llvm.org/

[PEP604]: https://www.python.org/dev/peps/pep-0604/

[rpython-instances]: https://rpython.readthedocs.io/en/latest/translation.html#user-defined-classes-and-instances
