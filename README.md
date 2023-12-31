# vekk
But at least read the README first!

`vekk` is a disappearingly small small vector optimization.

This project is an attempt to optimize Rust enum layout to make a small vector optimization.

Enum packing is suboptimal when abstracting using another struct:

```rust
enum E0 {
    A(A),
    B(u64),
}

struct A(u16, u64);
```

The enum `E0` takes up three words on a 64-bit architecture, because it must be possible to take the address of struct `A` and it must be two words because of alignment.

If instead the enum is reorganized like the following:

```rust
enum E1 {
    A(u16, u64),
    B(u64),
}
```

`E1` can be represented with two words.
It's not possible to take the address of the `E1::A(u16, u64)` variant tuple, so struct layout constraints don't apply to it.
This means it can made smaller than two words, leaving space for the enum discriminant.

This is taken advantage of to create a Small Vector Optimization, inspired by [TinyVec](https://docs.rs/tinyvec/latest/tinyvec/enum.TinyVec.html).
But TinyVec abstracts its two representations into two different structs (presumably for code reuse).
`Vekk` encodes its inline variant using this enum trick, thereby potentially saving some space.

`Vekk`'s heap representation uses [thin-vec](https://docs.rs/thin-vec/latest/thin_vec/).
