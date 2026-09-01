# Producing something the hardware will load

Between a compiler and the hardware there are four steps. `selfish` is all four, and each
one is a command you can run on its own - which matters, because when a file is rejected
the useful question is *which step produced the wrong bytes*.

```
   compiler output
        |  stamp     the platform identity no linker sets
        v
     module
        |  wrap      the signed-executable container
        v
   eboot.bin
        |  image     the filesystem the title mounts
        v
   image file
        |  pack      the package around it
        v
   package.pkg
```

## 1. Stamp the identity

```bash
selfish stamp module.elf --generation 5
```

Rewrites the file in place. No linker sets these fields, because no linker knows about
either console - so a freshly compiled ELF is not yet a module, and nothing about it looks
wrong until a loader refuses it.

`--library` stamps it as a shared library rather than an executable.

## 2. Wrap it in a container

```bash
selfish wrap module.elf --out eboot.bin --generation 5
```

Without `--out` it writes `eboot.bin` beside the input.

**`--generation` defaults to 4, and that is a measurement rather than a habit.** Every
container found inside real packages for the *current* console carries the *previous*
generation's magic - thirty-three of them, including a working homebrew store. Pass
`--generation 5` when you mean it; the default is what the evidence says is normal.

The container declares itself fake in the field the format provides for exactly that, and
its signature area is zero. No vendor signature is forged and none could be.

## 3. Build the filesystem image

```bash
selfish image --root ./files --out image.dat --content-id UP0000-TEST00001_00-0000000000000000
```

The files become a plain filesystem, wrapped in a `PFSC` container, carried as the single
file of a signed and encrypted outer filesystem.

**The content id is not optional, and getting it wrong is the quiet failure.** The image is
encrypted under a key derived from the content id and the passcode, so an image built under
one id cannot be opened by a package built under another - and nothing about the resulting
file looks wrong until a console tries to mount it.

`--passcode` defaults to the fake one, which is what you want unless you have a reason.

## 4. Pack it

```bash
selfish pack --dir ./files --out package.pkg --content-id UP0000-TEST00001_00-0000000000000000
```

`--dir` runs the whole chain - step 3 and step 4 together - which is the usual way in.
`--image` takes an image you already built, for when you are iterating on the package
around a filesystem that has not changed.

**Everything derivable is computed. Everything else must be handed in.** The entries this
project cannot yet compute are supplied with `--entry`, and the build **refuses rather than
inventing them**. A package that assembled itself by guessing an entry is a package that
installs and then fails somewhere with no connection to the guess.

A package's licence carries a signature under the published **debug** keyset, whose entire
purpose is the fake licences a non-retail package holds. Signing with it asserts "this is a
debug licence", which is true, and the licence says so in its own type field. The line is
not "never compute a signature" - it is never claim to be the vendor.

## When you do not know what an entry means

```bash
selfish derive package-a.pkg package-b.pkg package-c.pkg
```

Re-derives what a package's entries mean from packages you supply. This is the tool for
turning "there is an entry here nobody has named" into a row with a provenance, rather than
a guess written into a table.
