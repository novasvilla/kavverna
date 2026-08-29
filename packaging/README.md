# Packaging

## Building an Arch package now

```sh
cd packaging
makepkg -si
```

The `PKGBUILD` builds from the tagged release tarball, runs the library tests, and installs the
binary, the desktop entry and the icon. The tests that talk to a live compositor and a live
session bus are left out, since a build chroot has neither.

## Publishing to the AUR

Not done yet, and it needs an account rather than a change here. Registration is closed at the
time of writing while the AUR deals with automated sign-ups, and reopening is announced on
aur-general and the Arch news feed rather than on the register page. Once
[aur.archlinux.org](https://aur.archlinux.org) has an account with the machine's public key on
it:

```sh
git clone ssh://aur@aur.archlinux.org/kavverna.git aur-kavverna
cd aur-kavverna
cp ../packaging/PKGBUILD ../packaging/.SRCINFO .
git add PKGBUILD .SRCINFO
git commit -m "Add kavverna 0.2.1"
git push
```

## Every release after that

The version lives in three places and all three have to move together:

1. `version` in the workspace `Cargo.toml`
2. `pkgver` in `PKGBUILD`, with `sha256sums` recomputed from the new tarball
3. `.SRCINFO`, regenerated with `makepkg --printsrcinfo > .SRCINFO`

```sh
sha256sum <(curl -sL https://github.com/novasvilla/kavverna/archive/refs/tags/vX.Y.Z.tar.gz)
```

The fourth number in a version is the CI run that built the binary, so it is never written
down here: `KAVVERNA_BUILD` supplies it and a build made by hand reads zero.
