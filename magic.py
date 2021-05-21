import sys


BUILTINS = {float, int, str, object, bool, type, dict, set, list, tuple, isinstance, id}


DUNDERS = {
    "add",
    "sub",
    "pow",
    "mul",
    "eq",
    "ne",
    "and",
    "or",
    "lshift",
    "rshift",
    "div",
    "getattr",
    "getattribute",
    "getitem",
}


CTYPES = {
    "char",
    "byte",
    "double",
    "longdouble",
    "float",
    "int",
    "int8",
    "int32",
    "int64",
    "long",
    "longlong",
    "size_t",
    "ssize_t",
    "ubyte",
    "uint",
    "uint8",
    "uint16",
    "uint32",
    "uint64",
    "ulong",
    "ulonglong",
    "ushort",
    "void",
    "wchar",
    "bool",
}


print(
    "# DO NOT EDIT THIS FILE: automatically generated from 'magic.py' in the repository root.\n"
)

for builtin in BUILTINS:
    print(f"{builtin.__name__!s}")

for dunder in DUNDERS:
    print(f"__{dunder}__  # type: ignore")

    if dunder != "eq":
        print(f"__r{dunder}__  # type: ignore")

for ctype in CTYPES:
    print(f"c_{ctype}  # type: ignore")
    print(f"c_{ctype}_p # type: ignore")

# used by the interpreter

print("__value  # type: ignore")
