# ADR-0051: A mod's image is the package's thumbnail, encoded again

- **Status:** Accepted (2026-10-04)
- **Related:** #ADR-0024, #ADR-0041, #ADR-0003; `docs/design/ARCHITECTURE.md`, "Package" and
  "Untrusted input"

## Context

A Fantome archive holds the mod's picture at `META/image.png`. A mod manager shows it beside
the mod's name. A mod project names a thumbnail file in its config, and a `.modpkg` holds it as
the meta chunk `_meta_/thumbnail.webp`, a WebP. `ltk_mod_project`'s packer takes a WebP file as
it is and converts any other picture when it packs.

A run kept only the name, the size and the first bytes of the entry, so a repaired or ported
mod had no picture in a mod manager.

The entry's name promises a PNG. Authors put a JPEG or a WebP under it. The bytes are a mod's.

The options were:

- to write the archive's bytes into the project and let the packer convert them. A picture
  that does not convert then fails the pack, and the package is not written over its picture.
- to carry the picture only when it is a WebP already. Most are PNG.
- to decode the picture in the run and encode it again.

## Decision

The user decided (2026-10-04) that a run does not drop the mod's picture.

A run reads `META/image.png` whole, up to 5 MiB, which is the most the packer takes as a
thumbnail. `build` decodes it as a PNG, a JPEG or a WebP, in that order, and encodes it again
as WebP. The package holds the bytes the run encoded and never the bytes the archive shipped.
A writer puts them at `thumbnail.webp` in the project and names the file in the config, and
the packer stores them as the `.modpkg`'s thumbnail.

A picture that is not carried is never a reason to refuse or to fail the package. The run
raises `package.thumbnail-dropped`, a note, with why: the entry is over the size, the entry
does not read, the bytes decode as none of the three formats within the limits, the picture
does not encode, or the WebP is over 5 MiB.

The decoder runs under limits on the picture's width, its height and the memory it may ask
for, and where its panic is caught.

Only the first entry at that name is the picture. An entry at another name under `META/` is
not.

## Consequences

- The workspace depends on `image`, with its PNG, JPEG and WebP codecs and no other. These are
  the formats league-mod's own Fantome import reads.
- The WebP is lossless, so a JPEG grows when it is encoded again. One that grows over 5 MiB is
  dropped with the note.
- An animated picture keeps its first frame.
- The picture is read under the archive's budget of inflated bytes like any entry the run
  reads whole. An image entry that does not read raises nothing in `read`: the note of `build`
  says it.
- The report's package row holds the digest of the thumbnail. A report of a mod with a picture
  differs from one made before this decision by that field.
- A package taken through a repair again has no picture: `Run::again` reads the package's
  chunks, and the thumbnail is none of them.
