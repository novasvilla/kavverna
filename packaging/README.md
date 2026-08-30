# Packaging

## Building an Arch package now

```sh
cd packaging
makepkg -si
```

The `PKGBUILD` builds from the release's git tag, runs the library tests, and installs the
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
git commit -m "Add kavverna X.Y.Z"
git push
```

## Every release after that

The version lives in three places and all three have to move together:

1. `version` in the workspace `Cargo.toml`, and build afterwards, so the lock file moves with
   it. Committing a bump without building leaves `--locked` refusing the next CI run.
2. `pkgver` in `PKGBUILD`. The source is the git tag itself, so there is no checksum to
   recompute: a checksum for a tarball GitHub generates from the tag cannot exist in a commit
   made before the tag does.
3. `.SRCINFO`, regenerated with `makepkg --printsrcinfo > .SRCINFO`. CI diffs it against the
   definition and fails the build when they disagree.

The tag and `pkgver` are compared by the release workflow before anything is published, and
the built binary is asked its own version. The fourth number in a version is the CI run that
built the binary, so it is never written down here: `KAVVERNA_BUILD` supplies it and a build
made by hand reads zero.
