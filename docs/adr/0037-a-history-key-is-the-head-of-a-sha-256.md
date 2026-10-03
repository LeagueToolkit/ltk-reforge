# ADR-0037: A history key is the head of a SHA-256

- **Status:** Accepted (2026-10-03)
- **Related:** #ADR-0005, #ADR-0035; `docs/design/HISTORY.md`, "Keys"

## Context

A run asks the history index whether the game ever shipped a file, an object or a clip. The
question is asked by content: the SHA-256 of the file's bytes, of the object's bytes in its
bin, of the clip's bytes in its bank. The index holds one row per content, a few million of
each kind, and a run asks thousands of questions.

The census history writes a file's and an object's SHA-256 whole. It writes a clip's and a
norm key's as the leading eight bytes of the digest (#ADR-0035).

The options for the index were the whole digest (32 bytes a key), a head of it, or another hash
of the bytes.

## Decision

A key is the leading bytes of the SHA-256, in digest order:

| key | of | bytes |
|---|---|---|
| file key | a file's bytes | 16 |
| object key | an object's bytes in its bin, entry name included | 16 |
| norm key | an object's bytes in its bin, entry name left out | 8 |
| clip key | a clip's bytes in its bank | 8 |

A lookup compares the whole key. No filter and no shorter hash stands before it.

The norm key and the clip key are eight bytes because the census history writes them so. The
file key and the object key are sixteen because the index is the only thing that shortens them.

## Consequences

- A mod's own file is not called the game's by a collision. Among four million keys the chance
  of two contents sharing a 128-bit key is below 10^-25, and of two sharing a 64-bit key about
  10^-6.
- A 64-bit collision would call one clip, or one renamed object, the game's. A clip is asked
  by its bytes alone, so nothing else tells the two apart. An object found by its norm key is
  also compared by its class.
- A key is half or a quarter of the digest, and so is its index.
- The same four types are the model's (`FileKey`, `ObjectKey`, `NormKey`, `ClipKey`), so a key
  of one kind is not asked of another kind's table.
