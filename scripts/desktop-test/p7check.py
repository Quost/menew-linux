#!/usr/bin/env python3
"""Pixel checks of the annotation export (Phase 7). Reads the images with ImageMagick (`convert ... rgb:-`), so it
does not depend on how the export wrote the PNG.

  p7check.py blur ORIGINAL EXPORT x0,y0,x1,y1 [skip x0,y0,x1,y1 ...]
      inside the box every pixel is the average of its 10 x 10 block (counted from the top left corner, the block
      size is max(10, round(width / 90))); outside the box (and outside the skipped boxes) the image is identical.
  p7check.py marks ORIGINAL EXPORT MIN_CHANGED x0,y0,x1,y1 ... [probe x,y,r,g,b,tol ...]
      every pixel that changed is inside one of the boxes, at least MIN_CHANGED pixels changed, and each probe
      pixel has the color given (to the tolerance).

Prints one line with the numbers and exits 0 when the check holds, 1 otherwise.
"""
import subprocess
import sys


def read(path):
    w, h = map(int, subprocess.check_output(["identify", "-format", "%w %h", path + "[0]"]).decode().split())
    data = subprocess.check_output(["convert", path, "-alpha", "off", "-depth", "8", "rgb:-"])
    assert len(data) == w * h * 3, (path, len(data), w, h)
    return w, h, data


def box(arg):
    return tuple(int(v) for v in arg.split(","))


def inside(b, x, y):
    return b[0] <= x < b[2] and b[1] <= y < b[3]


def check_blur(orig, out, target, skips):
    w, h, a = read(orig)
    w2, h2, b = read(out)
    if (w, h) != (w2, h2):
        print("blur-check: FAIL size changed %dx%d -> %dx%d" % (w, h, w2, h2))
        return 1
    block = max(10, int(w / 90.0 + 0.5))
    bad_avg = bad_outside = changed = total = 0
    colors = set()
    cache = {}
    for y in range(h):
        for x in range(w):
            i = (y * w + x) * 3
            if inside(target, x, y):
                total += 1
                bx, by = x // block * block, y // block * block
                want = cache.get((bx, by))
                if want is None:
                    n = 0
                    s = [0, 0, 0]
                    for yy in range(by, min(by + block, h)):
                        for xx in range(bx, min(bx + block, w)):
                            j = (yy * w + xx) * 3
                            s[0] += a[j]; s[1] += a[j + 1]; s[2] += a[j + 2]
                            n += 1
                    want = cache[(bx, by)] = tuple((v + n // 2) // n for v in s)
                got = tuple(b[i:i + 3])
                colors.add(got)
                if got != want:
                    bad_avg += 1
                if got != tuple(a[i:i + 3]):
                    changed += 1
            elif not any(inside(k, x, y) for k in skips):
                if b[i:i + 3] != a[i:i + 3]:
                    bad_outside += 1
    ok = bad_avg == 0 and bad_outside == 0 and total > 0 and changed * 10 >= total * 9 and len(colors) >= 10
    print("blur-check: %s box=%s pixels=%d block=%d not-the-block-average=%d changed=%d (%.0f%%) distinct-colors-inside=%d changed-outside=%d"
          % ("OK" if ok else "FAIL", target, total, block, bad_avg, changed, 100.0 * changed / max(total, 1), len(colors), bad_outside))
    return 0 if ok else 1


def check_marks(orig, out, min_changed, boxes, probes):
    w, h, a = read(orig)
    w2, h2, b = read(out)
    if (w, h) != (w2, h2):
        print("marks-check: FAIL size changed")
        return 1
    changed = outside = 0
    for y in range(h):
        for x in range(w):
            i = (y * w + x) * 3
            if b[i:i + 3] != a[i:i + 3]:
                changed += 1
                if not any(inside(k, x, y) for k in boxes):
                    outside += 1
    bad = []
    for p in probes:
        x, y, r, g, bl, tol = (int(v) for v in p.split(","))
        i = (y * w + x) * 3
        got = tuple(b[i:i + 3])
        if max(abs(got[0] - r), abs(got[1] - g), abs(got[2] - bl)) > tol:
            bad.append("(%d,%d)=%s wanted %s" % (x, y, got, (r, g, bl)))
    ok = changed >= min_changed and outside == 0 and not bad
    print("marks-check: %s changed=%d (at least %d) changed-outside-the-boxes=%d probes=%d%s"
          % ("OK" if ok else "FAIL", changed, min_changed, outside, len(probes), (" bad: " + "; ".join(bad)) if bad else ""))
    return 0 if ok else 1


def main(argv):
    if len(argv) >= 5 and argv[1] == "blur":
        return check_blur(argv[2], argv[3], box(argv[4]), [box(s) for s in argv[5:]])
    if len(argv) >= 6 and argv[1] == "marks":
        rest = argv[5:]
        probes = []
        boxes = []
        if "probe" in rest:
            k = rest.index("probe")
            boxes, probes = [box(s) for s in rest[:k]], rest[k + 1:]
        else:
            boxes = [box(s) for s in rest]
        return check_marks(argv[2], argv[3], int(argv[4]), boxes, probes)
    print(__doc__)
    return 2


if __name__ == "__main__":
    sys.exit(main(sys.argv))
