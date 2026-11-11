"""Rebind a verified restored clone; the host launches only its new registration."""
import os
import stat
import sys
import uuid

def replace_marker(directory, source, target, owner=0):
    if source == target or any(str(uuid.UUID(i)) != i for i in (source, target)):
        raise RuntimeError("Invalid restore identity")
    marker = "environment-id"
    flags = os.O_NOFOLLOW | os.O_CLOEXEC | os.O_RDONLY | os.O_NONBLOCK
    old = os.open(marker, flags, dir_fd=directory)
    meta = os.fstat(old)
    if not stat.S_ISREG(meta.st_mode) or meta.st_uid != owner or meta.st_nlink != 1 or meta.st_mode & 0o022:
        raise RuntimeError("Unsafe marker")
    if os.read(old, 128) != (source + "\n").encode("ascii"):
        raise RuntimeError("Backup marker differs")
    temporary = "restored-" + target + ".pending"
    new = os.open(temporary, os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW | os.O_WRONLY, 0o644, dir_fd=directory)
    with os.fdopen(new, "wb") as out:
        out.write((target + "\n").encode("ascii"))
        out.flush()
        os.fsync(out.fileno())
    current = os.stat(marker, dir_fd=directory, follow_symlinks=False)
    if not os.path.samestat(meta, current):
        raise RuntimeError("Marker changed before commit")
    os.replace(temporary, marker, src_dir_fd=directory, dst_dir_fd=directory)
    os.fsync(directory)
    os.close(old)
    readback = os.open(marker, flags, dir_fd=directory)
    if os.read(readback, 128) != (target + "\n").encode("ascii"):
        raise RuntimeError("New marker verification failed")
    os.close(readback)


if __name__ == "__main__":
    source, target = sys.argv[1:]
    if os.geteuid() != 0:
        raise RuntimeError("Root in the managed recovery clone is required")
    directory_flags = os.O_DIRECTORY | os.O_NOFOLLOW | os.O_CLOEXEC | os.O_RDONLY
    directory = os.open("/", directory_flags)
    for name in ("etc", "openkoda"):
        child = os.open(name, directory_flags, dir_fd=directory)
        metadata = os.fstat(child)
        if metadata.st_uid != 0 or metadata.st_mode & 0o022:
            raise RuntimeError("Unsafe marker directory")
        os.close(directory)
        directory = child
    replace_marker(directory, source, target)
    os.close(directory)
    print(target)
