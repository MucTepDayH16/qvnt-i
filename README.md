# QVNT

[![build](https://img.shields.io/github/workflow/status/MucTepDayH16/qvnt-i/Rust?style=for-the-badge&logo=github&label=build/tests)](https://github.com/MucTepDayH16/qvnt-i/actions/workflows/rust.yml)
[![rustc](https://img.shields.io/badge/rustc-1.59.0+-blue?style=for-the-badge&logo=rust)](https://www.rust-lang.org)
[![crates.io](https://img.shields.io/crates/v/qvnt-i?style=for-the-badge&logo=hackthebox&logoColor=white)](https://crates.io/crates/qvnt-i)
[![docs.rs](https://img.shields.io/docsrs/qvnt?style=for-the-badge&logo=rust)](https://docs.rs/qvnt/)

### Advanced quantum computation simulator, written in *Rust*


## Features
1. Ability to simulate up to 64 qubits.
   Common machine with 4-16 Gb of RAM is able to simulate 26-28 qubits, which is enough for several study cases;
2. Set of 1- or 2-qubits operations to build your own quantum circuits;
3. Quantum operations are tested and debugged to be safe in use;
4. Circuit execution is accelerated using multithreading *Rayon* library;
5. Complex quantum registers manipulations: tensor product of two registers and aliases for qubit to simplify interaction with register.

___
## QVNT interpreter
### About
It is REPL interpreter, that could be used to process quantum operation without compiling code.
### Installation:
```shell
cargo install qvnt-i
```

### How to
Now, you are able to _'run'_ quantum simulator with OpenQASM language.
`*.qasm` files should be passed to interpreter via cli:
```shell
qvnt-i --input ./cirquit.qasm
|Q> :go
```
or via interpreter:
```shell
qvnt-i
|Q> :load ./cirquit.qasm
|Q> :go
```

Another way of running simulator is writing cirquit on OpenQASM language directly in REPL:
```shell
qvnt-i
|Q> qreg q[4];
|Q> creg c[4];
|Q> h q;
|Q> measure q -> c;
|Q> :go
|Q> :class
```
* `:go` - process the simulation;
* `:class` - acquire the result from classical register.

REPL is _lazy_: it only starts computation, if it encounters `:go`.
This example will shows the single number every time:
```shell
|Q> qreg q[4];
|Q> creg c[4];
|Q> h q;
|Q> measure q -> c;
|Q> :go
|Q> :class
|Q> :class
|Q> :class
|Q> :class
...
```
Unlike that, repeating `:go` will proceed with different result every time:
```shell
|Q> qreg q[4];
|Q> creg c[4];
|Q> h q;
|Q> measure q -> c;
|Q> :go
|Q> :class
|Q> :go
|Q> :class
|Q> :go
|Q> :class
...
```
### Commands
All commands should be preceeded with `:`.
Otherwise, REPL considers to parse line as OpenQASM source.
The full list of commands:
```ignore
loop N     Repeat following commands N time
tags TAG   Create TAG with current state
goto TAG   Swap current state to TAG's state
class      Show state of classical registers
polar      Show state of quantum registers in polar form
prob       Show state of quantum registers in probability form
ops        Snow current quantum operations queue
go         Start modulating quantum computer
reset      Clear current state
names      Show aliases for quantum and classical bits
load FILE  Load state from FILE according to QASM language script
help       Show this reference
quit       Exit interpreter
```


___
## License
Licensed under [MIT License](LICENSE.md)
