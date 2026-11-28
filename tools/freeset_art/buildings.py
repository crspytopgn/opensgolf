"""Buildings of the free set, put together from a small kit (walls with windows and doors, hipped and gabled roofs with
overhangs, porches, awnings, chimneys, steps, planters, paving) in each theme's style: Parkland white clapboard with dark
roofs, Desert adobe with flat roofs and red tiles, Tropical bungalows with thatch, Links grey stone with slate. World
units: a tile is 100; the building stands centred on the origin, its front (the clubhouse door) towards -x, -z."""
import numpy as np

from scene import Scene, box, ellipsoid, quad, tube
import trees


def c(*rgb):
    return np.array(rgb, float) / 255.0


STYLES = {
    # wall, trim, roof, roof edge, glass, door, awning a, awning b, base stone, paving, accent
    "park": dict(wall=c(236, 230, 214), trim=c(250, 250, 246), roof=c(84, 92, 104), roof2=c(70, 76, 88),
                 glass=c(70, 100, 130), door=c(110, 50, 44), aw1=c(46, 110, 70), aw2=c(240, 240, 232),
                 stone=c(170, 160, 146), pave=c(196, 188, 170), accent=c(46, 110, 70), lawn=c(96, 156, 70)),
    "desert": dict(wall=c(222, 170, 120), trim=c(244, 226, 196), roof=c(188, 86, 56), roof2=c(160, 70, 46),
                   glass=c(60, 96, 120), door=c(120, 72, 40), aw1=c(64, 140, 150), aw2=c(244, 230, 200),
                   stone=c(196, 150, 110), pave=c(214, 182, 140), accent=c(64, 140, 150), lawn=c(110, 160, 80)),
    "trop": dict(wall=c(244, 236, 214), trim=c(120, 84, 52), roof=c(196, 166, 100), roof2=c(170, 140, 82),
                 glass=c(56, 120, 140), door=c(56, 140, 150), aw1=c(236, 120, 70), aw2=c(250, 238, 210),
                 stone=c(220, 210, 190), pave=c(232, 222, 196), accent=c(40, 160, 170), lawn=c(84, 170, 70)),
    "links": dict(wall=c(170, 166, 156), trim=c(236, 232, 220), roof=c(70, 78, 92), roof2=c(58, 64, 76),
                  glass=c(60, 84, 104), door=c(60, 80, 64), aw1=c(120, 40, 50), aw2=c(236, 226, 206),
                  stone=c(140, 136, 128), pave=c(176, 170, 156), accent=c(120, 40, 50), lawn=c(116, 150, 80)),
}


def theme_style(path):
    low = path.lower()
    if "/desert/" in low:
        return "desert"
    if "/tropical/" in low:
        return "trop"
    if "/links/" in low:
        return "links"
    return "park"


class Kit:
    """Adds parts to a scene in one style."""

    def __init__(self, scene, style, rng):
        self.s = scene
        self.st = STYLES[style]
        self.style = style
        self.rng = rng

    def add(self, tris, col):
        self.s.add_tris(tris, col)

    def block(self, x0, z0, x1, z1, y0, y1, col=None, bands=0):
        """A box of wall from (x0, z0) to (x1, z1), y0 to y1; `bands` horizontal siding lines."""
        col = self.st["wall"] if col is None else col
        self.add(box([(x0 + x1) / 2, (y0 + y1) / 2, (z0 + z1) / 2], [x1 - x0, y1 - y0, z1 - z0]), col)
        # a plinth of the base stone at the foot, a soft band of shade along the foot (the ground's occlusion) and under
        # the eaves: the walls sit on the ground and under their roof instead of floating as flat boxes
        faces = (((x0, z0), (x1, z0)), ((x0, z1), (x1, z1)), ((x0, z0), (x0, z1)), ((x1, z0), (x1, z1)))
        for (a, b) in faces:
            if y0 < 1:
                self._band_on_face(a, b, y0, y0 + 3.5, self.st["stone"] * 0.82, 0.45)
                self._band_on_face(a, b, y0 + 3.5, y0 + 7, col * 0.9, 0.35)
            self._band_on_face(a, b, y1 - 4, y1, col * 0.8, 0.35)
        if bands:
            # thin darker lines along the outside faces
            for k in range(1, bands):
                y = y0 + (y1 - y0) * k / bands
                for (a, b) in (((x0, z0), (x1, z0)), ((x0, z1), (x1, z1)), ((x0, z0), (x0, z1)), ((x1, z0), (x1, z1))):
                    self._line_on_face(a, b, y, col * 0.88)

    def _band_on_face(self, a, b, y0, y1, col, off):
        """A band of colour from y0 to y1 just proud of the wall face from a to b (both sides)."""
        a = np.array([a[0], 0, a[1]], float)
        b = np.array([b[0], 0, b[1]], float)
        d = b - a
        n = np.array([-d[2], 0, d[0]])
        n = n / (np.linalg.norm(n) + 1e-9)
        for sgn in (1, -1):
            o = n * sgn * off
            self.add(quad(a + o + [0, y0, 0], b + o + [0, y0, 0], b + o + [0, y1, 0], a + o + [0, y1, 0]), col)

    def _line_on_face(self, a, b, y, col, t=0.7):
        a = np.array([a[0], y, a[1]], float)
        b = np.array([b[0], y, b[1]], float)
        d = b - a
        n = np.array([-d[2], 0, d[0]])
        n = n / (np.linalg.norm(n) + 1e-9)
        for sgn in (1, -1):
            o = n * sgn * 0.25
            self.add(quad(a + o, b + o, b + o + [0, t, 0], a + o + [0, t, 0]), col)

    def window(self, p, normal, w=14.0, h=18.0, frame=None, glass=None, shutters=None):
        """A window on a wall face: centre p (on the face), outward normal (x or z axis)."""
        frame = self.st["trim"] if frame is None else frame
        glass = self.st["glass"] if glass is None else glass
        p = np.asarray(p, float)
        n = np.asarray(normal, float)
        side = np.array([-n[2], 0, n[0]])
        up = np.array([0, 1.0, 0])
        def rect(cx, hw, hh, off, col):
            q = cx + n * off
            self.add(quad(q - side * hw - up * hh, q + side * hw - up * hh, q + side * hw + up * hh, q - side * hw + up * hh), col)
        rect(p, w / 2 + 1.6, h / 2 + 1.6, 0.6, frame)
        # a sill casting a little shade
        rect(p - up * (h / 2 + 1.6), w / 2 + 2.6, 1.0, 1.4, frame * 0.92)
        if self.rng.uniform() < 0.4:
            # a warm, lit room behind the glass, brighter at the top
            q = p + n * 0.9
            hw, hh = w / 2, h / 2
            self.s.add_tris(quad(q - side * hw - up * hh, q + side * hw - up * hh, q + side * hw + up * hh, q - side * hw + up * hh),
                            c(236, 176, 92), emissive=True)
            q = p + n * 1.0 + up * h * 0.2
            self.s.add_tris(quad(q - side * (hw - 0.5) - up * hh * 0.5, q + side * (hw - 0.5) - up * hh * 0.5,
                                 q + side * (hw - 0.5) + up * (hh * 0.8 - 0.5), q - side * (hw - 0.5) + up * (hh * 0.8 - 0.5)),
                            c(255, 222, 150), emissive=True)
        else:
            rect(p, w / 2, h / 2, 0.9, glass)
            # a light sky reflection on the upper half
            rect(p + up * h * 0.25, w / 2 - 0.5, h / 4 - 0.5, 1.0, np.minimum(glass * 1.45 + 0.08, 1))
        rect(p, 0.7, h / 2, 1.1, frame)
        rect(p, w / 2, 0.7, 1.1, frame)
        if shutters is not None:
            rect(p + side * (w / 2 + 4), 3, h / 2, 0.8, shutters)
            rect(p - side * (w / 2 + 4), 3, h / 2, 0.8, shutters)

    def door(self, p, normal, w=14.0, h=24.0, col=None):
        col = self.st["door"] if col is None else col
        p = np.asarray(p, float)
        n = np.asarray(normal, float)
        side = np.array([-n[2], 0, n[0]])
        up = np.array([0, 1.0, 0])
        def rect(cx, hw, h0, h1, off, cc):
            q = cx + n * off
            self.add(quad(q - side * hw + up * h0, q + side * hw + up * h0, q + side * hw + up * h1, q - side * hw + up * h1), cc)
        rect(p, w / 2 + 2, 0, h + 2, 0.6, self.st["trim"])
        rect(p, w / 2, 0, h, 1.0, col)
        rect(p, w / 2 - 2.5, h * 0.55, h - 3, 1.2, self.st["glass"] * 1.2)

    def windows_along(self, a, b, y, normal, n, w=14, h=18, shutters=None, skip=()):
        a = np.asarray(a, float)
        b = np.asarray(b, float)
        for k in range(n):
            if k in skip:
                continue
            t = (k + 0.5) / n
            p = a + (b - a) * t
            self.window([p[0], y, p[1]], normal, w, h, shutters=shutters)

    def hip_roof(self, x0, z0, x1, z1, y, h, over=6.0, col=None, ridge=None):
        """A hipped roof over the rectangle, rising h from y, with an overhang."""
        col = self.st["roof"] if col is None else col
        x0, z0, x1, z1 = x0 - over, z0 - over, x1 + over, z1 + over
        cx, cz = (x0 + x1) / 2, (z0 + z1) / 2
        dx, dz = (x1 - x0) / 2, (z1 - z0) / 2
        r = min(dx, dz)
        a = np.array([x0, y, z0])
        b = np.array([x1, y, z0])
        cc = np.array([x1, y, z1])
        d = np.array([x0, y, z1])
        if dx >= dz:
            r1 = np.array([cx - (dx - r), y + h, cz])
            r2 = np.array([cx + (dx - r), y + h, cz])
            faces = [(a, b, r2, r1), (cc, d, r1, r2), (d, a, r1, r1), (b, cc, r2, r2)]
        else:
            r1 = np.array([cx, y + h, cz - (dz - r)])
            r2 = np.array([cx, y + h, cz + (dz - r)])
            faces = [(a, b, r1, r1), (cc, d, r2, r2), (d, a, r1, r2), (b, cc, r2, r1)]
        for k, (p, q, rr, ss) in enumerate(faces):
            self.add(np.array([[p, q, rr], [p, rr, ss]]), col)
            # shingle rows: thin darker strips parallel to the eaves
            for t in (0.25, 0.5, 0.75):
                p2, q2 = p + (ss - p) * t, q + (rr - q) * t
                p3, q3 = p + (ss - p) * (t + 0.04), q + (rr - q) * (t + 0.04)
                lift = np.array([0, 0.25, 0])
                self.add(quad(p2 + lift, q2 + lift, q3 + lift, p3 + lift), col * 0.8)
            # a lighter cap along the hips and ridge
            for e0, e1 in ((q, rr), (p, ss), (rr, ss)):
                if np.linalg.norm(e1 - e0) > 1e-3:
                    dd = (e1 - e0) / np.linalg.norm(e1 - e0)
                    sd = np.cross(dd, [0, 1.0, 0])
                    sd = sd / (np.linalg.norm(sd) + 1e-9) * 1.0
                    self.add(quad(e0 - sd + [0, 0.4, 0], e1 - sd + [0, 0.4, 0], e1 + sd + [0, 0.4, 0], e0 + sd + [0, 0.4, 0]),
                             np.minimum(col * 1.18 + 0.03, 1))
        # eave fascia
        for p, q in ((a, b), (b, cc), (cc, d), (d, a)):
            self.add(quad(p, q, q - [0, 2.5, 0], p - [0, 2.5, 0]), self.st["trim"])

    def gable_roof(self, x0, z0, x1, z1, y, h, along_x=True, over=6.0, col=None, gable_col=None):
        col = self.st["roof"] if col is None else col
        gable_col = self.st["wall"] if gable_col is None else gable_col
        if along_x:
            cz = (z0 + z1) / 2
            e0, e1 = x0 - over, x1 + over
            a, b = np.array([e0, y, z0 - over]), np.array([e1, y, z0 - over])
            cc, d = np.array([e1, y, z1 + over]), np.array([e0, y, z1 + over])
            r1, r2 = np.array([e0, y + h, cz]), np.array([e1, y + h, cz])
            self.add(quad(a, b, r2, r1), col)
            self.add(quad(cc, d, r1, r2), col * 0.97)
            for x in (x0, x1):
                self.add(np.array([[[x, y, z0], [x, y, z1], [x, y + h * (1 - over / (cz - z0 + over) * 0), cz]]]), gable_col)
        else:
            cx = (x0 + x1) / 2
            e0, e1 = z0 - over, z1 + over
            a, b = np.array([x0 - over, y, e0]), np.array([x0 - over, y, e1])
            cc, d = np.array([x1 + over, y, e1]), np.array([x1 + over, y, e0])
            r1, r2 = np.array([cx, y + h, e0]), np.array([cx, y + h, e1])
            self.add(quad(a, b, r2, r1), col)
            self.add(quad(cc, d, r1, r2), col * 0.97)
            for z in (z0, z1):
                self.add(np.array([[[x0, y, z], [x1, y, z], [cx, y + h, z]]]), gable_col)

    def flat_roof(self, x0, z0, x1, z1, y, parapet=3.0, col=None):
        col = self.st["wall"] if col is None else col
        self.add(box([(x0 + x1) / 2, y + 0.5, (z0 + z1) / 2], [x1 - x0, 1, z1 - z0]), col * 0.9)
        t = 2.0
        for (a0, b0, a1, b1) in ((x0, z0, x1, z0 + t), (x0, z1 - t, x1, z1), (x0, z0, x0 + t, z1), (x1 - t, z0, x1, z1)):
            self.add(box([(a0 + a1) / 2, y + parapet / 2, (b0 + b1) / 2], [a1 - a0, parapet, b1 - b0]), col)

    def awning(self, p, normal, w, depth=9.0, y=22.0, drop=6.0, stripes=6):
        """A striped awning over a door or window."""
        p = np.asarray(p, float)
        n = np.asarray(normal, float)
        side = np.array([-n[2], 0, n[0]])
        for k in range(stripes):
            t0 = -w / 2 + w * k / stripes
            t1 = -w / 2 + w * (k + 1) / stripes
            col = self.st["aw1"] if k % 2 == 0 else self.st["aw2"]
            top0 = p + side * t0 + [0, y, 0]
            top1 = p + side * t1 + [0, y, 0]
            self.add(quad(top0, top1, top1 + n * depth - [0, drop, 0], top0 + n * depth - [0, drop, 0]), col)
            self.add(quad(top0 + n * depth - [0, drop, 0], top1 + n * depth - [0, drop, 0],
                          top1 + n * depth - [0, drop + 3, 0], top0 + n * depth - [0, drop + 3, 0]), col * 0.9)

    def porch(self, x0, z0, x1, z1, h=24.0, roof_h=8.0, posts=4, rail=True, col=None):
        """A covered porch: deck, posts, a shallow hipped roof."""
        trim = self.st["trim"]
        self.add(box([(x0 + x1) / 2, 1.5, (z0 + z1) / 2], [x1 - x0, 3, z1 - z0]), self.st["stone"])
        xs = np.linspace(x0 + 2, x1 - 2, posts)
        zs = np.linspace(z0 + 2, z1 - 2, posts)
        for x in xs:
            for z in (z0 + 2, z1 - 2):
                self.add(tube([x, 3, z], [x, h, z], 1.4, 1.4, 6), trim)
        for z in zs:
            for x in (x0 + 2, x1 - 2):
                self.add(tube([x, 3, z], [x, h, z], 1.4, 1.4, 6), trim)
        self.hip_roof(x0, z0, x1, z1, h, roof_h, over=3.0, col=col)

    def steps(self, x0, z0, x1, z1, n=3, h=3.0, axis="x", down=-1):
        for k in range(n):
            t = k / n
            if axis == "x":
                w = (x1 - x0)
                xa = x0 + w * t if down > 0 else x0
                xb = x1 if down > 0 else x1 - w * t
                self.add(box([(xa + xb) / 2, h * (n - k) / 2, (z0 + z1) / 2], [xb - xa, h * (n - k), z1 - z0]), self.st["stone"] * (1 - 0.03 * k))
            else:
                w = (z1 - z0)
                za = z0 + w * t if down > 0 else z0
                zb = z1 if down > 0 else z1 - w * t
                self.add(box([(x0 + x1) / 2, h * (n - k) / 2, (za + zb) / 2], [x1 - x0, h * (n - k), zb - za]), self.st["stone"] * (1 - 0.03 * k))

    def chimney(self, x, z, y, h=16.0, w=7.0):
        self.add(box([x, y + h / 2, z], [w, h, w]), self.st["stone"] * 0.9)
        self.add(box([x, y + h + 1, z], [w + 2, 2, w + 2]), self.st["stone"] * 0.75)

    def planter(self, x, z, w=10.0, d=10.0, flowers=True):
        self.add(box([x, 3, z], [w, 6, d]), self.st["stone"])
        rng = self.rng
        for k in range(3):
            p = [x + rng.uniform(-w / 3, w / 3), 8, z + rng.uniform(-d / 3, d / 3)]
            col = c(70, 130, 56) if not flowers or k == 0 else [c(220, 70, 90), c(250, 210, 80), c(200, 120, 220)][k % 3]
            self.s.add_ball(p, 3.6, col)

    def shrub(self, x, z, r=7.0, col=None):
        col = c(66, 120, 52) if col is None else col
        rng = self.rng
        for k in range(3):
            self.s.add_ball([x + rng.uniform(-r / 2, r / 2), r * 0.7 + rng.uniform(0, r / 3), z + rng.uniform(-r / 2, r / 2)],
                            r * rng.uniform(0.55, 0.75), col * rng.uniform(0.9, 1.1))

    def tree(self, x, z, h=60.0, kind="broad"):
        rng = self.rng
        if kind == "palm":
            t = trees.palm(rng, h, fronds=8)
        elif kind == "pine":
            t = trees.conifer(rng, h, width=0.22)
        elif kind == "cactus":
            t = trees.saguaro(rng, h, arms=2)
        else:
            t = trees.broadleaf(rng, h)
        self.s.extend(t.transformed(1.0, move=[x, 0, z]))

    def deckchair(self, x, z, yaw=0.0, col=None):
        """A striped deckchair (a sight gag by the clubhouse: someone is taking it easy)."""
        col = self.st["aw1"] if col is None else col
        R = np.array([[np.cos(np.radians(yaw)), 0, np.sin(np.radians(yaw))], [0, 1, 0],
                      [-np.sin(np.radians(yaw)), 0, np.cos(np.radians(yaw))]])
        P = lambda p: np.asarray(p, float) @ R.T + [x, 0, z]
        wood = c(190, 150, 100)
        for sx in (-5, 5):
            self.add(tube(P([sx, 0, -8]), P([sx, 13, 4]), 0.6, 0.6, 4), wood)
            self.add(tube(P([sx, 0, 6]), P([sx, 6, -2]), 0.6, 0.6, 4), wood)
        for k in range(5):
            t0, t1 = k / 5, (k + 1) / 5
            a0 = np.array([0, 6, -2]) + (np.array([0, 1, 10]) - np.array([0, 6, -2])) * 0
            # the sling: from the front rail sagging down to the seat and up the back
            pts = [np.array([0, 5.5, 6]), np.array([0, 3.2, -1]), np.array([0, 12.5, 3.6])]
            def at(t):
                return pts[0] * (1 - t) ** 2 + pts[1] * 2 * t * (1 - t) + pts[2] * t * t
            p0, p1 = at(t0), at(t1)
            cc = col if k % 2 == 0 else self.st["aw2"]
            self.add(quad(P(p0 + [-4.6, 0, 0]), P(p0 + [4.6, 0, 0]), P(p1 + [4.6, 0, 0]), P(p1 + [-4.6, 0, 0])), cc)

    def golf_cart(self, x, z, yaw=0.0):
        """A golf cart parked on the lawn: white body, a striped canopy on posts, black seats, a bag of clubs behind."""
        a = np.radians(yaw)
        R = np.array([[np.cos(a), 0, np.sin(a)], [0, 1, 0], [-np.sin(a), 0, np.cos(a)]])
        P = lambda p: np.asarray(p, float) @ R.T + [x, 0, z]
        def bx(cen, size, col):
            self.add(box([0, 0, 0], size) @ R.T + P(cen), col)
        white, dark = c(244, 244, 238), c(40, 42, 46)
        bx([0, 7, 0], [20, 6, 34], white)
        bx([0, 12, 12], [20, 6, 8], white)
        bx([0, 12.5, -2], [18, 3, 12], dark)
        bx([0, 19, -8], [18, 11, 2.5], dark)
        for sx in (-8.5, 8.5):
            for sz in (-14, 9):
                self.add(tube(P([sx, 9, sz]), P([sx, 33, sz]), 0.7, 0.7, 4), c(200, 200, 200))
        bx([0, 34, -2], [22, 1.8, 30], self.st["aw1"])
        bx([0, 35, -2], [22, 0.6, 10], self.st["aw2"])
        for sx in (-10, 10):
            for sz in (-11, 11):
                self.add(tube(P([sx - 1.5, 4, sz]), P([sx + 1.5, 4, sz]), 4, 4, 8), dark)
        self.add(tube(P([0, 8, -19]), P([0, 26, -22]), 3.4, 3.4, 8), c(200, 60, 50))
        for k in range(3):
            self.add(tube(P([k - 1, 26, -22]), P([k - 1.5, 30, -23]), 1.1, 1.1, 5), c(220, 220, 220))

    def picnic(self, x, z):
        """A picnic blanket in red and white checks with a basket on it."""
        n = 4
        for i in range(n):
            for j in range(n):
                col = c(214, 50, 50) if (i + j) % 2 == 0 else c(248, 246, 240)
                x0, z0 = x - 16 + i * 8, z - 12 + j * 6
                self.add(quad([x0, 0.8, z0], [x0 + 8, 0.8, z0], [x0 + 8, 0.8, z0 + 6], [x0, 0.8, z0 + 6]), col)
        self.add(box([x + 4, 4, z + 2], [9, 7, 7]), c(196, 150, 90))
        self.add(tube([x + 0.5, 7.5, z + 2], [x + 7.5, 7.5, z + 2], 0.6, 0.6, 4), c(150, 106, 60))
        self.s.add_ball([x - 7, 2.2, z - 4], 2.2, c(230, 40, 40))

    def gnome(self, x, z):
        """A garden gnome: boots, a blue coat, a white beard, a big red pointy hat."""
        self.add(box([x, 1.5, z + 0.5], [5, 3, 6]), c(70, 50, 40))
        self.add(tube([x, 3, z], [x, 10, z], 4, 3.4, 10), c(50, 90, 190))
        self.s.add_ball([x, 12, z + 1.0], 2.9, c(240, 200, 170))
        self.add(tube([x, 9.5, z + 2.6], [x, 13.4, z + 2.4], 3.0, 1.8, 8), c(246, 246, 240))
        self.add(tube([x, 13.5, z], [x + 1, 24, z - 1.5], 3.8, 0.3, 10), c(220, 40, 40))

    def flagpole(self, x, z, h=60.0, col=None):
        col = self.st["accent"] if col is None else col
        self.add(tube([x, 0, z], [x, h, z], 0.8, 0.6, 5), c(220, 220, 220))
        self.add(quad([x, h - 1, z], [x + 12, h - 3, z + 2], [x + 12, h - 11, z + 2], [x, h - 10, z]), col)

    def umbrella(self, x, z, r=10.0, h=18.0, col=None):
        col = self.st["aw1"] if col is None else col
        self.add(tube([x, 0, z], [x, h, z], 0.6, 0.6, 4), c(200, 200, 200))
        n = 8
        for k in range(n):
            a0, a1 = 2 * np.pi * k / n, 2 * np.pi * (k + 1) / n
            cc = col if k % 2 == 0 else self.st["aw2"]
            self.add(np.array([[[x, h + 4, z], [x + np.cos(a0) * r, h - 1, z + np.sin(a0) * r],
                                [x + np.cos(a1) * r, h - 1, z + np.sin(a1) * r]]]), cc)
        self.add(box([x, 7, z], [8, 1.2, 8]), c(240, 240, 236))
        self.add(tube([x, 0, z], [x, 7, z], 0.8, 0.8, 4), c(200, 200, 200))


# ---- ground (the flat base layers) --------------------------------------------------------------------------------------


def ground_patch(s, st, x0, z0, x1, z1, col, y=0.4, tiles=0, rng=None):
    """A flat paved or planted area on the ground (for base layers); `tiles` > 0 draws a grid of slightly varied slabs."""
    if tiles <= 0:
        s.add_tris(quad([x0, y, z0], [x1, y, z0], [x1, y, z1], [x0, y, z1]), col)
        return
    nx = max(1, int(round((x1 - x0) / tiles)))
    nz = max(1, int(round((z1 - z0) / tiles)))
    for i in range(nx):
        for j in range(nz):
            a0 = x0 + (x1 - x0) * i / nx
            a1 = x0 + (x1 - x0) * (i + 1) / nx
            b0 = z0 + (z1 - z0) * j / nz
            b1 = z0 + (z1 - z0) * (j + 1) / nz
            k = 1.0 + (rng.uniform(-0.04, 0.04) if rng is not None else 0)
            s.add_tris(quad([a0 + 0.5, y, b0 + 0.5], [a1 - 0.5, y, b0 + 0.5], [a1 - 0.5, y, b1 - 0.5], [a0 + 0.5, y, b1 - 0.5]),
                       col * k)
            s.add_tris(quad([a0, y - 0.1, b0], [a1, y - 0.1, b0], [a1, y - 0.1, b1], [a0, y - 0.1, b1]), col * 0.8)


# ---- buildings -------------------------------------------------------------------------------------------------------------


def clubhouse(style, level, rng):
    """The clubhouse: a two storey house with a corner entrance towards -x -z, a porch and a tower (level 2 larger)."""
    body = Scene()
    base = Scene()
    k = Kit(body, style, rng)
    st = k.st
    big = level >= 2
    W = 120 if big else 96   # half extents of the main block
    D = 70 if big else 56
    H = 34                    # storey height
    floors = 2
    # the paved forecourt and lawn on the base layer
    ground_patch(base, st, -W - 34, -D - 34, 10, -D + 10, st["pave"], y=0.6, tiles=12, rng=rng)
    ground_patch(base, st, -W - 34, -D - 34, -W + 10, 10, st["pave"], y=0.6, tiles=12, rng=rng)
    if style == "desert":
        # a hacienda: two storeys of warm adobe under red tile hipped roofs, an arcade along the front, a tiled tower
        k.block(-W, -D, W, D, 0, H * 2, st["wall"])
        k.hip_roof(-W, -D, W, D, H * 2, 26, over=6, col=st["roof"])
        tw = 24
        k.block(-W - 6, -D - 6, -W - 6 + tw * 2, -D - 6 + tw * 2, 0, H * 2 + 22, st["wall"] * 1.03)
        k.hip_roof(-W - 6, -D - 6, -W - 6 + tw * 2, -D - 6 + tw * 2, H * 2 + 22, 22, over=4, col=st["roof2"])
        # the arcade: arches of pillars and a tiled lean-to roof along the front
        for x in np.linspace(-W + 52, W - 6, 6):
            k.add(box([x, (H - 6) / 2, -D - 14], [5, H - 6, 5]), st["trim"])
        k.add(box([(-W + 48 + W) / 2, H - 4, -D - 14], [W * 2 - 48, 4, 6]), st["trim"])
        k.hip_roof(-W + 48, -D - 18, W, -D, H, 6, over=2, col=st["roof"])
        # beam ends under the eaves
        for x in np.linspace(-W + 8, W - 8, 12):
            k.add(box([x, H * 2 - 3, -D - 1.5], [2.2, 2.2, 3]), c(110, 76, 50))
    elif style == "trop":
        # bungalow on a raised deck with a big thatched hip roof and an open veranda
        k.add(box([0, 3, 0], [W * 2 + 30, 6, D * 2 + 30]), c(150, 110, 70))
        k.block(-W, -D, W, D, 6, H + 6, st["wall"])
        k.hip_roof(-W - 10, -D - 10, W + 10, D + 10, H + 6, 48 if big else 40, over=8, col=st["roof"])
        for x in np.linspace(-W - 12, W + 12, 7):
            for z in (-D - 12, D + 12):
                k.add(tube([x, 6, z], [x, H + 6, z], 1.8, 1.8, 6), st["trim"])
        H = H + 6
        floors = 1
    elif style == "links":
        # grey stone with slate gables and chimneys
        k.block(-W, -D, W, D, 0, H * 2, st["wall"], bands=0)
        k.gable_roof(-W, -D, W, D, H * 2, 38, along_x=True, over=5, col=st["roof"], gable_col=st["wall"])
        k.block(-W - 2, -D - 26, -W + 40, -D + 2, 0, H * 2, st["wall"] * 1.02)
        k.gable_roof(-W - 2, -D - 26, -W + 40, -D + 2, H * 2, 30, along_x=False, over=4, col=st["roof"] * 0.96,
                     gable_col=st["wall"] * 1.02)
        k.chimney(W * 0.6, 0, H * 2 + 20, 20)
        k.chimney(-W * 0.2, 0, H * 2 + 20, 20)
    else:
        # parkland: white clapboard, dark hipped roof, a corner tower and a porch
        k.block(-W, -D, W, D, 0, H * 2, st["wall"], bands=10)
        k.hip_roof(-W, -D, W, D, H * 2, 34, over=7)
        tw = 26
        k.block(-W - 8, -D - 8, -W - 8 + tw * 2, -D - 8 + tw * 2, 0, H * 2 + 16, st["wall"] * 1.02, bands=11)
        k.hip_roof(-W - 8, -D - 8, -W - 8 + tw * 2, -D - 8 + tw * 2, H * 2 + 16, 30, over=5, col=st["roof2"])
        k.porch(-W + 44, -D - 26, W - 10, -D, h=H - 4, roof_h=7)
        k.chimney(W * 0.55, D * 0.3, H * 2 + 12, 22)
        # dormers on the front slope
        for x in np.linspace(-W + 60, W - 30, 3 if big else 2):
            k.block(x - 9, -D + 6, x + 9, -D + 26, H * 2, H * 2 + 18, st["wall"])
            k.gable_roof(x - 9, -D + 6, x + 9, -D + 26, H * 2 + 18, 9, along_x=False, over=2, col=st["roof2"])
            k.window([x, H * 2 + 10, -D + 6], (0, 0, -1), 9, 11)
    # windows on the four sides of the main block
    for f in range(floors):
        y = 18 + f * H
        sh = st["accent"] if style in ("park", "links") else None
        k.windows_along((-W + 40, -D), (W, -D), y, (0, 0, -1), 5 if big else 4, shutters=sh)
        k.windows_along((-W, D), (W, D), y, (0, 0, 1), 6 if big else 5, shutters=sh)
        k.windows_along((-W, -D + 40), (-W, D), y, (-1, 0, 0), 3, shutters=sh)
        k.windows_along((W, -D), (W, D), y, (1, 0, 0), 3, shutters=sh)
    # the entrance: a door on the corner block facing -x, steps, an awning, planters and a flag
    if style == "park":
        k.door([-W - 8, 0, -D - 8 + 26], (-1, 0, 0), 16, 26)
        k.awning([-W - 8, 0, -D - 8 + 26], (-1, 0, 0), 22, y=30)
        k.steps(-W - 22, -D + 6, -W - 8, -D + 30, n=3, axis="x")
    elif style == "trop":
        k.door([-W, 6, -D + 30], (-1, 0, 0), 18, 26)
        k.steps(-W - 30, -D + 18, -W - 15, -D + 42, n=2, axis="x")
    elif style == "desert":
        k.door([-W - 2, 0, -D + 16], (-1, 0, 0), 16, 28)
        k.awning([-W - 2, 0, -D + 16], (-1, 0, 0), 20, y=32)
    else:
        k.door([-W - 2, 0, -D - 12], (-1, 0, 0), 16, 26)
    k.planter(-W - 20, -D - 20)
    k.planter(-W - 20, -D + 44)
    k.planter(-W + 50, -D - 20)
    k.flagpole(-W - 30, -D - 30, 70)
    # out on the lawn: a parasol and two deckchairs
    k.umbrella(W + 30, -D - 34, 13, 20)
    k.deckchair(W + 18, -D - 40, yaw=200)
    k.deckchair(W + 42, -D - 36, yaw=160)
    # a golf cart parked by the entrance, a picnic on the side lawn and a gnome guarding the planter
    k.golf_cart(-W - 36, D - 10, yaw=10)
    k.picnic(W - 20, D + 34)
    k.gnome(-W - 32, -D + 58)
    if style == "trop":
        k.tree(W + 26, -D - 20, 90, "palm")
        k.tree(-W - 10, D + 24, 80, "palm")
    elif style == "desert":
        k.tree(W + 20, -D - 26, 50, "cactus")
        k.tree(-W - 24, D + 10, 70, "palm")
    elif style == "links":
        k.shrub(W + 14, -D - 14, 9)
        k.shrub(-W - 12, D + 12, 9)
    else:
        k.tree(W + 26, D + 20, 70, "broad")
        k.shrub(W + 12, -D - 14, 9)
        k.shrub(-W - 14, D + 10, 9)
    return body, base


def roof_for(k, style, x0, z0, x1, z1, y, h, along_x=True):
    """The theme's roof over a block: hipped slate or shingle, flat with a parapet in the desert, thatch in the tropics."""
    if style == "desert":
        k.flat_roof(x0, z0, x1, z1, y, 3.5, k.st["wall"])
    elif style == "links":
        k.gable_roof(x0, z0, x1, z1, y, h, along_x=along_x, over=4, col=k.st["roof"], gable_col=k.st["wall"])
    elif style == "trop":
        k.hip_roof(x0 - 4, z0 - 4, x1 + 4, z1 + 4, y, h * 1.3, over=7, col=k.st["roof"])
    else:
        k.hip_roof(x0, z0, x1, z1, y, h, over=6)


def sign_board(k, p, normal, w, h, y, col):
    """A sign board above a door: a coloured panel with a light border and a few light 'letters'."""
    p = np.asarray(p, float)
    n = np.asarray(normal, float)
    side = np.array([-n[2], 0, n[0]])
    up = np.array([0, 1.0, 0])
    q = p + n * 1.5 + up * y
    k.add(quad(q - side * (w / 2 + 1) - up, q + side * (w / 2 + 1) - up, q + side * (w / 2 + 1) + up * (h + 1),
               q - side * (w / 2 + 1) + up * (h + 1)), k.st["trim"])
    q2 = q + n * 0.3
    k.add(quad(q2 - side * w / 2, q2 + side * w / 2, q2 + side * w / 2 + up * h, q2 - side * w / 2 + up * h), col)
    for i in range(5):
        t = -w / 2 + w * (i + 0.6) / 5.4
        q3 = q2 + n * 0.3 + up * h * 0.3
        k.add(quad(q3 + side * t, q3 + side * (t + w / 9), q3 + side * (t + w / 9) + up * h * 0.4, q3 + side * t + up * h * 0.4),
              k.st["trim"])


def cart_model(k, x, z, rot=0.0):
    """A parked golf cart (white with a dark seat and roof)."""
    s = Scene()
    w, L = 12.0, 24.0
    s.add_tris(box([0, 5, 0], [w, 5, L]), c(240, 240, 236))
    s.add_tris(box([0, 9, L * 0.3], [w, 5, 6]), c(240, 240, 236))
    s.add_tris(box([0, 10, -2], [w * 0.9, 3, 10]), c(60, 66, 74))
    for xx in (-w / 2 + 1, w / 2 - 1):
        for zz in (-L * 0.4, L * 0.3):
            s.add_tris(tube([xx, 7, zz], [xx, 30, zz], 0.6, 0.6, 4), c(190, 190, 190))
    s.add_tris(box([0, 31, -2], [w + 2, 1.5, L * 0.9]), k.st["aw1"])
    for xx in (-w / 2, w / 2):
        for zz in (-L * 0.34, L * 0.34):
            s.add_tris(tube([xx - 1, 3, zz], [xx + 1, 3, zz], 3, 3, 8), c(40, 40, 44))
    a = np.radians(rot)
    r = np.array([[np.cos(a), 0, np.sin(a)], [0, 1, 0], [-np.sin(a), 0, np.cos(a)]])
    k.s.extend(s.transformed(1.0, rot=r, move=[x, 0, z]))


def shop(style, rng, kind="pro", level=1):
    """Pro shop, snack bar, cart garage, driving range shop: a one or two storey block with a signed front."""
    body, base = Scene(), Scene()
    k = Kit(body, style, rng)
    st = k.st
    big = level >= 2
    W, D = {"pro": (50, 36), "snack": (30, 24), "garage": (64, 38), "range": (34, 26)}[kind]
    if big:
        W, D = W * 1.25, D * 1.15
    H = 30 if kind != "garage" else 34
    floors = 2 if (big and kind == "pro") else 1
    ground_patch(base, st, -W - 26, -D - 30, W + 14, -D + 4, st["pave"], y=0.6, tiles=12, rng=rng)
    k.block(-W, -D, W, D, 0, H * floors, st["wall"], bands=6 * floors if style == "park" else 0)
    roof_for(k, style, -W, -D, W, D, H * floors, 22, along_x=True)
    front = (0, 0, -1)
    if kind == "garage":
        for x in np.linspace(-W + 18, W - 18, 3):
            p = np.array([x, 0, -D])
            k.add(quad(p + [-12, 0, -0.6], p + [12, 0, -0.6], p + [12, 24, -0.6], p + [-12, 24, -0.6]), st["trim"])
            k.add(quad(p + [-10.5, 0, -1], p + [10.5, 0, -1], p + [10.5, 22.5, -1], p + [-10.5, 22.5, -1]), c(150, 150, 146))
            for yy in range(3, 22, 4):
                k.add(quad(p + [-10.5, yy, -1.2], p + [10.5, yy, -1.2], p + [10.5, yy + 0.8, -1.2], p + [-10.5, yy + 0.8, -1.2]),
                      c(120, 120, 118))
        cart_model(k, -W + 10, -D - 26, 10)
        cart_model(k, -W + 34, -D - 28, -5)
        sign_board(k, [0, 0, -D], front, 40, 7, H * floors - 2 if style != "desert" else H - 6, st["accent"])
    else:
        k.door([-W * 0.35, 0, -D], front, 15, 24)
        k.awning([-W * 0.35, 0, -D], front, 20, y=28)
        k.windows_along((-W * 0.1, -D), (W, -D), 15, front, 2, w=18, h=14)
        k.windows_along((-W, D), (W, D), 16, (0, 0, 1), 3)
        k.windows_along((-W, -D), (-W, D), 16, (-1, 0, 0), 2)
        k.windows_along((W, -D), (W, D), 16, (1, 0, 0), 2)
        if floors > 1:
            k.windows_along((-W, -D), (W, -D), 16 + H, front, 4)
        sign_board(k, [W * 0.3, 0, -D], front, 30, 6, H * floors - 1 if style != "desert" else H - 6,
                   st["accent"] if kind != "snack" else c(200, 60, 50))
    if kind == "snack":
        # a serving hatch and tables with umbrellas on the patio
        k.add(box([W * 0.3, 12, -D - 3], [24, 2, 6]), st["trim"])
        k.umbrella(-W - 10, -D - 18, 9, 18)
        k.umbrella(W * 0.4, -D - 22, 9, 18, col=st["accent"])
    if kind == "pro":
        k.planter(-W - 12, -D - 12)
        k.flagpole(W + 12, -D - 14, 54)
    if kind == "range":
        # a little ball dispenser and a bucket stack by the door
        k.add(box([W + 8, 9, -D + 6], [8, 18, 8]), st["accent"])
        k.add(box([W + 8, 19, -D + 6], [9, 2, 9]), st["trim"])
    if style == "trop":
        k.tree(W + 18, D + 10, 70, "palm")
    elif style == "desert":
        k.tree(W + 14, -D - 12, 40, "cactus")
    else:
        k.shrub(W + 10, -D - 8, 7)
        k.shrub(-W - 10, D + 8, 7)
    return body, base


def hotel(style, rng, level=1):
    body, base = Scene(), Scene()
    k = Kit(body, style, rng)
    st = k.st
    big = level >= 2
    W, D = (120, 60) if big else (96, 52)
    floors = 5 if big else 4
    H = 26
    ground_patch(base, st, -W - 30, -D - 60, W + 30, -D + 2, st["pave"], y=0.6, tiles=14, rng=rng)
    # a pool on the forecourt
    ground_patch(base, st, W * 0.15, -D - 52, W * 0.85, -D - 16, c(240, 240, 236), y=0.8)
    ground_patch(base, st, W * 0.15 + 3, -D - 49, W * 0.85 - 3, -D - 19, c(70, 170, 210), y=1.0)
    k.block(-W, -D, W, D, 0, H * floors, st["wall"], bands=0)
    roof_for(k, style, -W, -D, W, D, H * floors, 26)
    for f in range(floors):
        y = 14 + f * H
        k.windows_along((-W, -D), (W, -D), y, (0, 0, -1), 8 if big else 6, w=12, h=14)
        k.windows_along((-W, D), (W, D), y, (0, 0, 1), 8 if big else 6, w=12, h=14)
        k.windows_along((-W, -D), (-W, D), y, (-1, 0, 0), 4, w=12, h=14)
        k.windows_along((W, -D), (W, D), y, (1, 0, 0), 4, w=12, h=14)
        if f > 0:
            # balconies: a slab and a light rail along the front
            k.add(box([0, f * H + 1, -D - 3], [W * 2 - 10, 2, 6]), st["trim"])
            k.add(box([0, f * H + 6, -D - 5.6], [W * 2 - 10, 1, 0.8]), st["trim"])
    # the entrance canopy
    k.door([-W * 0.5, 0, -D], (0, 0, -1), 20, 22)
    k.add(box([-W * 0.5, 26, -D - 12], [34, 2, 24]), st["accent"])
    for x in (-W * 0.5 - 15, -W * 0.5 + 15):
        k.add(tube([x, 0, -D - 22], [x, 26, -D - 22], 1.3, 1.3, 6), st["trim"])
    sign_board(k, [-W * 0.5, 0, -D], (0, 0, -1), 30, 6, 30, st["accent"])
    for x in (-W - 14, W + 14):
        if style == "trop" or style == "desert":
            k.tree(x, -D - 20, 80, "palm")
        else:
            k.tree(x, -D - 16, 60, "broad")
    k.umbrella(W * 0.3, -D - 56, 8, 16)
    k.umbrella(W * 0.7, -D - 56, 8, 16, col=st["accent"])
    return body, base


def putting(style, rng, level=1):
    """A putting green's starter hut, which stands on the lot's corner tile: the game draws the green itself as terrain,
    and the flags and the upgraded green's bridge as sprites of their own (sg_core::objects::putting_green)."""
    body, base = Scene(), Scene()
    k = Kit(body, style, rng)
    st = k.st
    big = level >= 2
    W, D = (20, 16) if big else (16, 13)
    H = 24 if big else 20
    ground_patch(base, st, -W - 10, -D - 14, W + 8, D + 8, st["pave"], y=0.6, tiles=6, rng=rng)
    k.block(-W, -D, W, D, 0, H, st["wall"], bands=4 if style == "park" else 0)
    roof_for(k, style, -W, -D, W, D, H, 14 if big else 12, along_x=True)
    front = (0, 0, -1)
    k.door([-W * 0.3, 0, -D], front, 9, 17)
    k.awning([-W * 0.3, 0, -D], front, 12, y=19)
    k.windows_along((W * 0.1, -D), (W, -D), 10, front, 1, w=10, h=9)
    k.windows_along((-W, -D), (-W, D), 10, (-1, 0, 0), 1)
    # a rack of putters by the door
    k.add(box([W * 0.45, 6, -D - 4], [8, 12, 3]), st["trim"])
    if big:
        k.flagpole(W + 7, -D - 6, 40)
    if style == "trop":
        k.tree(W + 8, D + 4, 50, "palm")
    elif style == "desert":
        k.tree(W + 6, -D - 8, 30, "cactus")
    else:
        k.shrub(W + 6, D + 4, 5)
        k.shrub(-W - 6, -D - 6, 5)
    return body, base


def tennis(style, rng, level=1):
    body, base = Scene(), Scene()
    k = Kit(body, style, rng)
    W, D = 60, 110
    surface = c(184, 96, 70) if style in ("park", "links") else c(70, 130, 170)
    base.add_tris(quad([-W - 8, 0.5, -D - 8], [W + 8, 0.5, -D - 8], [W + 8, 0.5, D + 8], [-W - 8, 0.5, D + 8]), surface * 0.85)
    base.add_tris(quad([-W, 0.6, -D], [W, 0.6, -D], [W, 0.6, D], [-W, 0.6, D]), surface)
    white = c(244, 244, 240)
    for (a, b) in (((-W, -D), (W, -D)), ((-W, D), (W, D)), ((-W, -D), (-W, D)), ((W, -D), (W, D)), ((-W * 0.75, -D), (-W * 0.75, D)),
                   ((W * 0.75, -D), (W * 0.75, D)), ((-W * 0.75, -D * 0.55), (W * 0.75, -D * 0.55)),
                   ((-W * 0.75, D * 0.55), (W * 0.75, D * 0.55)), ((0, -D * 0.55), (0, D * 0.55))):
        d = np.array([b[0] - a[0], 0, b[1] - a[1]])
        n = np.array([-d[2], 0, d[0]]) / (np.linalg.norm(d) + 1e-9) * 0.9
        p0, p1 = np.array([a[0], 0.7, a[1]]), np.array([b[0], 0.7, b[1]])
        base.add_tris(quad(p0 - n, p1 - n, p1 + n, p0 + n), white)
    # the net and its posts
    k.add(tube([-W - 4, 0, 0], [-W - 4, 9, 0], 0.8, 0.8, 4), c(80, 80, 80))
    k.add(tube([W + 4, 0, 0], [W + 4, 9, 0], 0.8, 0.8, 4), c(80, 80, 80))
    k.add(quad([-W - 4, 2, 0], [W + 4, 2, 0], [W + 4, 9, 0], [-W - 4, 9, 0]), c(40, 44, 48))
    k.add(quad([-W - 4, 8.2, 0.1], [W + 4, 8.2, 0.1], [W + 4, 9.2, 0.1], [-W - 4, 9.2, 0.1]), white)
    if level >= 2:
        # a little stand of seats at the side
        for r in range(3):
            k.add(box([-W - 24 - r * 7, 3 + r * 4, 0], [7, 6 + r * 8, 80]), k.st["trim"] * (1 - 0.05 * r))
    return body, base


def construction(style, rng):
    """A house being built: a timber frame on a slab."""
    body, base = Scene(), Scene()
    k = Kit(body, style, rng)
    W, D, H = 46, 34, 28
    k.add(box([0, 1.5, 0], [W * 2, 3, D * 2]), c(170, 166, 156))
    wood = c(196, 156, 104)
    for x in np.linspace(-W, W, 6):
        for z in (-D, D):
            k.add(box([x, H / 2 + 3, z], [2.4, H, 2.4]), wood)
    for z in np.linspace(-D, D, 4):
        for x in (-W, W):
            k.add(box([x, H / 2 + 3, z], [2.4, H, 2.4]), wood)
    for y in (3 + H * 0.5, 3 + H):
        for z in (-D, D):
            k.add(box([0, y, z], [W * 2, 2, 2.2]), wood * 0.95)
        for x in (-W, W):
            k.add(box([x, y, 0], [2.2, 2, D * 2]), wood * 0.95)
    # roof trusses
    for x in np.linspace(-W, W, 6):
        k.add(tube([x, 3 + H, -D], [x, 3 + H + 20, 0], 1.0, 1.0, 4), wood)
        k.add(tube([x, 3 + H, D], [x, 3 + H + 20, 0], 1.0, 1.0, 4), wood)
    k.add(box([W + 16, 6, -D + 6], [14, 12, 10]), c(150, 120, 80))
    return body, base


def house(style, rng, variant=0):
    """A family home by the course."""
    body, base = Scene(), Scene()
    k = Kit(body, style, rng)
    st = k.st
    W, D, H = 44 + 6 * variant, 32, 26
    ground_patch(base, st, -W - 16, -D - 20, -W + 14, -D + 2, st["pave"], y=0.6, tiles=10, rng=rng)
    walls = [st["wall"], c(206, 170, 140), c(196, 214, 226), c(236, 220, 180)][variant % 4]
    k.block(-W, -D, W, D, 0, H * (1 + variant % 2), walls, bands=6 if style == "park" else 0)
    hh = H * (1 + variant % 2)
    if style == "desert":
        k.flat_roof(-W, -D, W, D, hh, 3.5, walls)
        k.add(box([W * 0.4, hh + 8, D * 0.2], [W * 0.6, 16, D]), walls * 0.97)
    elif style == "trop":
        k.hip_roof(-W - 4, -D - 4, W + 4, D + 4, hh, 28, over=6, col=st["roof"])
    else:
        k.gable_roof(-W, -D, W, D, hh, 22, along_x=True, over=5, col=[st["roof"], c(150, 70, 56), c(80, 100, 120)][variant % 3],
                     gable_col=walls)
        k.chimney(W * 0.5, D * 0.3, hh + 10, 16, 6)
    k.door([-W * 0.3, 0, -D], (0, 0, -1), 13, 22)
    k.windows_along((-W * 0.05, -D), (W, -D), 14, (0, 0, -1), 2, shutters=st["accent"] if style in ("park", "links") else None)
    k.windows_along((-W, -D), (-W, D), 14, (-1, 0, 0), 2)
    k.windows_along((W, -D), (W, D), 14, (1, 0, 0), 2)
    k.windows_along((-W, D), (W, D), 14, (0, 0, 1), 3)
    if variant % 2:
        k.windows_along((-W, -D), (W, -D), 14 + H, (0, 0, -1), 3)
    k.shrub(-W - 8, -D + 12, 6)
    k.shrub(W + 8, -D - 6, 6)
    if style == "trop":
        k.tree(W + 14, D, 70, "palm")
    return body, base


def landmark_castle(style, rng, level=1):
    body, base = Scene(), Scene()
    k = Kit(body, style, rng)
    stone = c(150, 146, 136)
    W = 56 if level == 1 else 70
    k.block(-W, -W, W, W, 0, 60, stone)
    # crenellations
    for x in np.linspace(-W, W, 9):
        for z in (-W, W):
            k.add(box([x, 64, z], [7, 8, 5]), stone * 1.03)
    for z in np.linspace(-W, W, 9):
        for x in (-W, W):
            k.add(box([x, 64, z], [5, 8, 7]), stone * 1.03)
    for x, z in ((-W, -W), (W, -W), (-W, W), (W, W)):
        k.add(tube([x, 0, z], [x, 82, z], 14, 13, 10), stone * 0.97)
        k.add(tube([x, 82, z], [x, 110, z], 15, 0.5, 10), c(70, 78, 92))
    k.door([0, 0, -W], (0, 0, -1), 18, 30, c(80, 56, 40))
    k.windows_along((-W, -W), (W, -W), 40, (0, 0, -1), 3, w=6, h=12)
    k.windows_along((-W, -W), (-W, W), 40, (-1, 0, 0), 3, w=6, h=12)
    k.flagpole(0, 0, 100, c(200, 50, 60))
    return body, base


def church(style, rng, level=1):
    body, base = Scene(), Scene()
    k = Kit(body, style, rng)
    stone = c(176, 170, 156)
    W, D = 30, 70
    k.block(-W, -D, W, D, 0, 40, stone)
    k.gable_roof(-W, -D, W, D, 40, 30, along_x=False, over=4, col=c(70, 78, 92), gable_col=stone)
    k.block(-16, -D - 30, 16, -D + 2, 0, 80, stone * 1.02)
    k.add(np.array([[[-18, 80, -D - 32], [18, 80, -D - 32], [0, 130, -D - 14]], [[18, 80, -D - 32], [18, 80, 4 - D], [0, 130, -D - 14]],
                    [[18, 80, 4 - D], [-18, 80, 4 - D], [0, 130, -D - 14]], [[-18, 80, 4 - D], [-18, 80, -D - 32], [0, 130, -D - 14]]]),
          c(70, 78, 92))
    k.door([0, 0, -D - 30], (0, 0, -1), 14, 26, c(90, 60, 40))
    k.windows_along((-W, -D), (-W, D), 24, (-1, 0, 0), 4, w=8, h=20)
    k.windows_along((W, -D), (W, D), 24, (1, 0, 0), 4, w=8, h=20)
    return body, base


def stable(style, rng, level=1):
    body, base = Scene(), Scene()
    k = Kit(body, style, rng)
    red = c(160, 60, 50)
    W, D = 70, 34
    k.block(-W, -D, W, D, 0, 30, red)
    k.gable_roof(-W, -D, W, D, 30, 24, along_x=True, over=4, col=c(80, 84, 90), gable_col=red)
    for x in np.linspace(-W + 14, W - 14, 5):
        p = np.array([x, 0, -D])
        k.add(quad(p + [-8, 0, -0.6], p + [8, 0, -0.6], p + [8, 20, -0.6], p + [-8, 20, -0.6]), c(240, 236, 226))
        k.add(quad(p + [-6.5, 0, -0.9], p + [6.5, 0, -0.9], p + [6.5, 18.5, -0.9], p + [-6.5, 18.5, -0.9]), red * 0.8)
    # a paddock fence on the base
    for x in np.linspace(-W - 30, W + 30, 9):
        k.add(tube([x, 0, -D - 50], [x, 12, -D - 50], 0.8, 0.8, 4), c(220, 214, 200))
    k.add(box([0, 10, -D - 50], [W * 2 + 60, 1.2, 1.2]), c(220, 214, 200))
    k.add(box([0, 5, -D - 50], [W * 2 + 60, 1.2, 1.2]), c(220, 214, 200))
    return body, base


def pub(style, rng, level=1):
    body, base = Scene(), Scene()
    k = Kit(body, style, rng)
    stone = c(214, 206, 188)
    W, D = 46 if level == 1 else 60, 30
    k.block(-W, -D, W, D, 0, 28, stone)
    k.hip_roof(-W, -D, W, D, 28, 26, over=6, col=c(180, 150, 90))
    k.door([-W * 0.2, 0, -D], (0, 0, -1), 13, 22, c(60, 80, 64))
    k.windows_along((-W, -D), (W, -D), 14, (0, 0, -1), 4, w=10, h=10)
    k.chimney(W - 8, 0, 34, 20)
    sign_board(k, [W * 0.4, 0, -D], (0, 0, -1), 22, 6, 22, c(120, 40, 50))
    k.umbrella(-W - 14, -D - 16, 8, 16)
    return body, base


def marina(style, rng, level=1):
    body, base = Scene(), Scene()
    k = Kit(body, style, rng)
    wood = c(170, 130, 90)
    # a jetty of planks out over the water (the base) and a boathouse
    for i in range(12):
        z = -60 + i * 9
        base.add_tris(quad([-12, 2, z], [12, 2, z], [12, 2, z + 8], [-12, 2, z + 8]), wood * (1 - 0.04 * (i % 2)))
    k.block(20, -30, 70, 20, 0, 26, k.st["wall"])
    roof_for(k, style, 20, -30, 70, 20, 26, 16)
    k.door([20, 0, -5], (-1, 0, 0), 16, 20)
    # a moored boat
    hull = Scene()
    pts = [np.array([np.cos(a) * 9, 0, np.sin(a) * 26]) for a in np.linspace(0, 2 * np.pi, 15)]
    for i in range(14):
        hull.add_tris(np.array([[[0, 7, 0], pts[i] + [0, 7, 0], pts[i + 1] + [0, 7, 0]]]), c(240, 240, 236))
        hull.add_tris(quad(pts[i] + [0, 7, 0], pts[i + 1] + [0, 7, 0], pts[i + 1] * 0.7 + [0, 1, 0], pts[i] * 0.7 + [0, 1, 0]),
                      c(40, 70, 120))
    hull.add_tris(box([0, 11, -2], [10, 6, 12]), c(240, 240, 236))
    k.s.extend(hull.transformed(1.0, move=[-30, 0, -20]))
    return body, base


def airstrip(style, rng, level=1):
    body, base = Scene(), Scene()
    k = Kit(body, style, rng)
    L = 220 if level >= 2 else 180
    base.add_tris(quad([-24, 0.6, -L], [24, 0.6, -L], [24, 0.6, L], [-24, 0.6, L]), c(90, 92, 96))
    for z in np.arange(-L + 20, L - 10, 30):
        base.add_tris(quad([-1.5, 0.8, z], [1.5, 0.8, z], [1.5, 0.8, z + 14], [-1.5, 0.8, z + 14]), c(240, 240, 236))
    # the hangar: an arched roof
    hx = 70
    n = 10
    for i in range(n):
        a0, a1 = np.pi * i / n, np.pi * (i + 1) / n
        p0 = np.array([hx + np.cos(a0) * 34, np.sin(a0) * 34, -40])
        p1 = np.array([hx + np.cos(a1) * 34, np.sin(a1) * 34, -40])
        k.add(quad(p0, p1, p1 + [0, 0, 80], p0 + [0, 0, 80]), c(170, 176, 182) * (1 - 0.03 * (i % 2)))
    k.add(np.array([[[hx - 34, 0, -40], [hx + 34, 0, -40], [hx, 34, -40]]]), c(150, 156, 160))
    k.add(quad([hx - 18, 0, -40.5], [hx + 18, 0, -40.5], [hx + 18, 24, -40.5], [hx - 18, 24, -40.5]), c(70, 74, 80))
    # a small plane on the apron
    pl = Scene()
    pl.add_tris(tube([0, 8, -16], [0, 8, 18], 4, 2, 8), c(240, 240, 236))
    pl.add_tris(box([0, 9, 2], [52, 1.2, 9]), c(220, 60, 50))
    pl.add_tris(box([0, 9, -14], [18, 1, 5]), c(220, 60, 50))
    pl.add_tris(box([0, 13, -14], [1, 8, 5]), c(220, 60, 50))
    k.s.extend(pl.transformed(1.0, move=[-2, 0, -60]))
    return body, base


def range_platform(style, rng, top=True):
    """The tee platform of a driving range: a long mat strip with dividers (top) on a kerb (base)."""
    body, base = Scene(), Scene()
    k = Kit(body, style, rng)
    if top:
        k.add(box([0, 1.5, 0], [70, 3, 26]), c(80, 150, 70))
        for x in np.linspace(-30, 30, 5):
            k.add(box([x, 6, 6], [1.2, 8, 12]), k.st["trim"])
    else:
        base.add_tris(quad([-40, 0.5, -16], [40, 0.5, -16], [40, 0.5, 16], [-40, 0.5, 16]), k.st["pave"])
    return body, base


def distance_sign(style, rng, yards=100):
    body, base = Scene(), Scene()
    k = Kit(body, style, rng)
    k.add(tube([0, 0, 0], [0, 26, 0], 1.0, 1.0, 5), c(220, 220, 220))
    col = {100: c(220, 60, 50), 200: c(240, 200, 60), 300: c(60, 110, 200), 400: c(240, 240, 240)}.get(yards, c(220, 60, 50))
    k.add(box([0, 30, 0], [14, 10, 1.5]), col)
    k.add(box([0, 30, 0.9], [10, 6, 0.4]), c(250, 250, 250))
    return body, base


# ---- the resort amenities: casino, spa, swim club, theme park ----------------------------------------------------------------

WATER = c(64, 172, 214)
GOLD = c(232, 186, 72)


def disc(s, cx, cz, r, y, col, n=24, squash=1.0, wob=0.0, ph=0.0):
    """A flat round (or wobbly, kidney-ish) patch at height y: triangles into scene s."""
    ang = np.linspace(0, 2 * np.pi, n + 1)
    rr = r * (1 + wob * np.sin(2 * ang + ph))
    pts = [np.array([cx + np.cos(a) * q, y, cz + np.sin(a) * q * squash]) for a, q in zip(ang, rr)]
    s.add_tris(np.array([[np.array([cx, y, cz]), pts[i + 1], pts[i]] for i in range(n)]), col)
    return pts


def pool(s, x0, z0, x1, z1, y=0.8, rim=4.0, rim_col=None, water=WATER):
    """A rectangular pool: a light stone surround and the water, with a lighter band along the near edges."""
    rim_col = c(240, 236, 226) if rim_col is None else rim_col
    s.add_tris(quad([x0 - rim, y, z0 - rim], [x1 + rim, y, z0 - rim], [x1 + rim, y, z1 + rim], [x0 - rim, y, z1 + rim]), rim_col)
    s.add_tris(quad([x0, y + 0.2, z0], [x1, y + 0.2, z0], [x1, y + 0.2, z1], [x0, y + 0.2, z1]), water * 0.82)
    s.add_tris(quad([x0 + 2.5, y + 0.3, z0 + 2.5], [x1 - 2.5, y + 0.3, z0 + 2.5], [x1 - 2.5, y + 0.3, z1 - 2.5],
                    [x0 + 2.5, y + 0.3, z1 - 2.5]), water)
    # lane ropes or sun glints: a few light streaks across the water
    for k in range(1, 4):
        z = z0 + (z1 - z0) * k / 4
        s.add_tris(quad([x0 + 4, y + 0.4, z - 0.4], [x1 - 4, y + 0.4, z - 0.4], [x1 - 4, y + 0.4, z + 0.4], [x0 + 4, y + 0.4, z + 0.4]),
                   np.minimum(water * 1.35 + 0.1, 1))


def lagoon(s, cx, cz, r, y=0.8, squash=0.8, ph=0.0):
    """A free-form pool: a sandy rim, deep water and a light shallow ring."""
    disc(s, cx, cz, r + 5, y, c(244, 232, 200), 28, squash, 0.12, ph)
    disc(s, cx, cz, r, y + 0.2, WATER * 0.86, 28, squash, 0.12, ph)
    disc(s, cx, cz, r * 0.72, y + 0.3, np.minimum(WATER * 1.12, 1), 28, squash, 0.12, ph)


def lounger(k, x, z, yaw=0.0, col=None):
    """A sun lounger: a white frame and a striped cushion, its back raised."""
    col = k.st["aw1"] if col is None else col
    a = np.radians(yaw)
    R = np.array([[np.cos(a), 0, np.sin(a)], [0, 1, 0], [-np.sin(a), 0, np.cos(a)]])
    P = lambda p: np.asarray(p, float) @ R.T + [x, 0, z]
    k.add(box([0, 0, 0], [8, 2, 16]) @ R.T + P([0, 4, 2]), c(244, 244, 240))
    k.add(box([0, 0, 0], [7, 1, 14]) @ R.T + P([0, 5.4, 2]), col)
    k.add(quad(P([-4, 5, -6]), P([4, 5, -6]), P([4, 12, -10]), P([-4, 12, -10])), col * 0.92)
    for sx in (-3.5, 3.5):
        for sz in (-4, 8):
            k.add(tube(P([sx, 0, sz]), P([sx, 4, sz]), 0.5, 0.5, 4), c(220, 220, 220))


def lamp(k, x, z, h=24.0):
    """A street lamp: a dark post and a lit globe."""
    k.add(tube([x, 0, z], [x, h, z], 0.8, 0.6, 5), c(50, 52, 58))
    k.s.add_tris(ellipsoid([x, h + 2, z], [2.4, 2.4, 2.4], 8, 5), c(255, 236, 170), emissive=True)


def bulbs(k, a, b, n, col=None):
    """A row of n lit bulbs from a to b (marquee lights)."""
    col = c(255, 230, 140) if col is None else col
    a, b = np.asarray(a, float), np.asarray(b, float)
    for i in range(n):
        p = a + (b - a) * (i + 0.5) / n
        k.s.add_tris(ellipsoid(p, [1.1, 1.1, 1.1], 6, 4), col, emissive=True)


def dome(k, cx, y, cz, r, col, ribs=12):
    """A dome (half an ellipsoid) with a little finial."""
    t = ellipsoid([cx, y, cz], [r, r * 0.9, r], ribs, 8)
    t = t[np.all(t[:, :, 1] >= y - 0.01, axis=1)]
    k.add(t, col)
    k.add(tube([cx, y + r * 0.88, cz], [cx, y + r * 0.88 + r * 0.45, cz], r * 0.08, r * 0.02, 6), GOLD)


def palms(k, pts, h=80.0):
    for x, z in pts:
        k.tree(x, z, h * k.rng.uniform(0.85, 1.1), "palm")


def pylon_sign(k, x, z, h, w, col, globe=False):
    """A tall roadside sign: a post, a lit panel framed with bulbs, a star (or a globe) on top."""
    k.add(box([x, h / 2, z], [5, h, 5]), c(70, 72, 80))
    y0, y1 = h, h + w * 0.7
    k.add(box([x, (y0 + y1) / 2, z], [w + 4, y1 - y0 + 4, 4]), GOLD)
    k.s.add_tris(box([x, (y0 + y1) / 2, z], [w, y1 - y0, 4.6]), col, emissive=True)
    for zz in (z - 2.6, z + 2.6):
        bulbs(k, [x - w / 2 - 1, y0 - 1, zz], [x + w / 2 + 1, y0 - 1, zz], 9)
        bulbs(k, [x - w / 2 - 1, y1 + 1, zz], [x + w / 2 + 1, y1 + 1, zz], 9)
        # light 'letters' on the panel
        for i in range(5):
            t = -w / 2 + w * (i + 0.7) / 5.4
            k.s.add_tris(quad([x + t, y0 + (y1 - y0) * 0.3, zz * 1.0 + (zz - z) * 0.2], [x + t + w / 8, y0 + (y1 - y0) * 0.3, zz + (zz - z) * 0.2],
                              [x + t + w / 8, y0 + (y1 - y0) * 0.72, zz + (zz - z) * 0.2], [x + t, y0 + (y1 - y0) * 0.72, zz + (zz - z) * 0.2]),
                         c(255, 246, 210), emissive=True)
    if globe:
        k.s.add_tris(ellipsoid([x, y1 + 10, z], [8, 8, 8], 12, 8), c(255, 214, 90), emissive=True)
    else:
        k.add(np.array([[[x - 6, y1 + 3, z], [x + 6, y1 + 3, z], [x, y1 + 14, z]]]), GOLD)


def lit_windows(k, a, b, y0, y1, normal, cols, rows, warm=0.65):
    """A grid of windows on a wall face from a to b (x, z), between heights y0 and y1, most of them lit."""
    a, b = np.asarray(a, float), np.asarray(b, float)
    n = np.asarray(normal, float)
    side = (b - a) / np.linalg.norm(b - a)
    side3 = np.array([side[0], 0, side[1]])
    for i in range(cols):
        for j in range(rows):
            p = a + (b - a) * (i + 0.5) / cols
            y = y0 + (y1 - y0) * (j + 0.5) / rows
            hw = np.linalg.norm(b - a) / cols * 0.3
            hh = (y1 - y0) / rows * 0.32
            q = np.array([p[0], y, p[1]]) + n * 0.8
            lit = k.rng.uniform() < warm
            col = c(255, 214, 130) if lit else c(70, 100, 130)
            k.s.add_tris(quad(q - side3 * hw - [0, hh, 0], q + side3 * hw - [0, hh, 0], q + side3 * hw + [0, hh, 0],
                              q - side3 * hw + [0, hh, 0]), col, emissive=lit)


def casino(style, rng, level=1):
    """The desert's casino: a grand adobe hall with a golden dome, wings with lit arcades round a forecourt with a fountain,
    a canopied entrance lit with bulbs and a tall roadside sign; the upgrade is bigger and adds a hotel tower."""
    body, base = Scene(), Scene()
    k = Kit(body, style, rng)
    st = k.st
    big = level >= 2
    m = 1.2 if big else 1.0
    HW, HD, H = 130 * m, 52 * m, 52       # the hall: x -HW..HW, z 0..2 HD
    WX, WD, WH = 50 * m, 84 * m, 34       # the wings: x +-(HW - WX)..+-HW, z -WD..0
    roof = c(206, 150, 112)
    # the forecourt between the wings: warm paving, a red carpet to the door, flower beds round a big fountain
    ground_patch(base, st, -HW - 16, -WD - 34, HW + 16, 2 * HD + 12, st["pave"], y=0.6, tiles=18, rng=rng)
    base.add_tris(quad([-11, 0.8, -WD - 30], [11, 0.8, -WD - 30], [11, 0.8, 0], [-11, 0.8, 0]), c(176, 40, 46))
    fz = -WD * 0.5
    for x in (-HW + WX + 22, HW - WX - 22):
        disc(base, x, fz, 14, 0.8, c(96, 70, 50), 16)
        for a in np.linspace(0, 2 * np.pi, 8)[:-1]:
            base.add_tris(ellipsoid([x + np.cos(a) * 8, 1.6, fz + np.sin(a) * 8], [3, 1.8, 3], 6, 4),
                          [c(232, 70, 90), c(250, 200, 70), c(240, 120, 60)][int(a * 2) % 3])
    fx = 0
    disc(base, fx, fz, 30, 0.8, c(226, 214, 192), 28)
    disc(base, fx, fz, 25, 1.0, WATER, 28)
    disc(base, fx, fz, 18, 1.1, np.minimum(WATER * 1.15, 1), 28)
    # the fountain's bowls and its jet
    k.add(tube([fx, 0, fz], [fx, 12, fz], 3.5, 2.8, 10), c(232, 222, 202))
    k.add(tube([fx, 12, fz], [fx, 14, fz], 10, 11, 14), c(232, 222, 202))
    k.add(tube([fx, 14, fz], [fx, 22, fz], 1.6, 1.4, 6), c(232, 222, 202))
    k.add(tube([fx, 22, fz], [fx, 23.5, fz], 5, 5.5, 12), c(232, 222, 202))
    k.s.add_tris(ellipsoid([fx, 27, fz], [2.6, 6, 2.6], 8, 5), c(220, 244, 255), emissive=True)
    if big:
        # the hotel tower rising from the back of the hall
        TW, TD, TH = 70, 40, 236
        tz = HD * 1.15
        tower = c(244, 238, 226)
        k.block(-TW, tz - TD, TW, tz + TD, H, TH, tower)
        for f in range(1, 10):
            y = H + (TH - H) * f / 10
            for (x0, z0, x1, z1) in ((-TW, tz - TD, TW, tz - TD), (-TW, tz + TD, TW, tz + TD)):
                k.add(box([0, y, z0], [TW * 2 + 1.5, 1.6, 1.5]), st["trim"])
        lit_windows(k, (-TW, tz - TD), (TW, tz - TD), H + 6, TH - 8, (0, 0, -1), 10, 10, 0.4)
        lit_windows(k, (-TW, tz + TD), (TW, tz + TD), H + 6, TH - 8, (0, 0, 1), 10, 10, 0.4)
        lit_windows(k, (-TW, tz - TD), (-TW, tz + TD), H + 6, TH - 8, (-1, 0, 0), 6, 10, 0.4)
        lit_windows(k, (TW, tz - TD), (TW, tz + TD), H + 6, TH - 8, (1, 0, 0), 6, 10, 0.4)
        k.flat_roof(-TW, tz - TD, TW, tz + TD, TH, 4, tower)
        for (x0, z0, x1, z1) in ((-TW - 1, tz - TD - 1, TW + 1, tz - TD + 2), (-TW - 1, tz + TD - 2, TW + 1, tz + TD + 1),
                                 (-TW - 1, tz - TD - 1, -TW + 2, tz + TD + 1), (TW - 2, tz - TD - 1, TW + 1, tz + TD + 1)):
            k.add(box([(x0 + x1) / 2, TH + 6, (z0 + z1) / 2], [x1 - x0, 5, z1 - z0]), GOLD)
        k.add(box([TW * 0.4, TH + 8, tz], [18, 12, 14]), c(150, 146, 140))
    # the hall: adobe with a stone plinth, two rows of lit windows, a flat tiled roof with a parapet and skylights
    k.block(-HW, 0, HW, 2 * HD, 0, H, st["wall"])
    k.flat_roof(-HW, 0, HW, 2 * HD, H, 5, roof)
    k.add(box([0, H - 2, -0.8], [HW * 2 + 2, 3, 1.4]), GOLD)
    lit_windows(k, (-HW, 0), (-HW + WX, 0), 8, H - 8, (0, 0, -1), 2, 2, 0.8)
    lit_windows(k, (HW - WX, 0), (HW, 0), 8, H - 8, (0, 0, -1), 2, 2, 0.8)
    lit_windows(k, (-HW + WX, 0), (-22, 0), 8, H - 8, (0, 0, -1), 4, 2, 0.85)
    lit_windows(k, (22, 0), (HW - WX, 0), 8, H - 8, (0, 0, -1), 4, 2, 0.85)
    lit_windows(k, (-HW, 0), (-HW, 2 * HD), 8, H - 8, (-1, 0, 0), 6, 2, 0.6)
    lit_windows(k, (HW, 0), (HW, 2 * HD), 8, H - 8, (1, 0, 0), 6, 2, 0.6)
    lit_windows(k, (-HW, 2 * HD), (HW, 2 * HD), 8, H - 8, (0, 0, 1), 14, 2, 0.6)
    for x in (-HW * 0.55, HW * 0.55):
        k.add(box([x, H + 2.5, HD * 1.3], [26, 3, 18]), c(150, 196, 214))
        k.add(box([x, H + 1.5, HD * 1.3], [28, 2, 20]), c(236, 226, 206))
    # the dome on a drum over the entrance
    dr = 34 * m
    k.add(tube([0, H, HD * 0.75], [0, H + 16, HD * 0.75], dr, dr, 22), st["trim"])
    lit_windows(k, (-dr * 0.7, HD * 0.75 - dr), (dr * 0.7, HD * 0.75 - dr), H + 3, H + 14, (0, 0, -1), 6, 1, 0.9)
    dome(k, 0, H + 16, HD * 0.75, dr, GOLD, 18)
    # the wings: lower, with an arcade of columns along the forecourt and lit windows behind it
    for sx in (-1, 1):
        x0, x1 = (HW - WX) * sx, HW * sx
        xa, xb = min(x0, x1), max(x0, x1)
        k.block(xa, -WD, xb, 0, 0, WH, st["wall"])
        k.flat_roof(xa, -WD, xb, 0, WH, 4, roof)
        inner = x0
        lit_windows(k, (inner, -WD), (inner, 0), 6, WH - 8, (-sx, 0, 0), 5, 1, 0.9)
        for z in np.linspace(-WD + 6, -6, 6):
            k.add(tube([inner - sx * 9, 0, z], [inner - sx * 9, WH - 6, z], 1.8, 1.8, 8), st["trim"])
        k.add(box([inner - sx * 5, WH - 4, -WD / 2], [11, 3, WD]), st["trim"])
        lit_windows(k, (xa, -WD), (xb, -WD), 6, WH - 8, (0, 0, -1), 3, 1, 0.8)
        lit_windows(k, (x1, -WD), (x1, 0), 6, WH - 8, (sx, 0, 0), 5, 1, 0.8)
    # the entrance: tall bright glass doors under a red canopy on columns with bulbs round its edge
    ew = 34
    k.s.add_tris(quad([-ew / 2, 0, -1], [ew / 2, 0, -1], [ew / 2, 32, -1], [-ew / 2, 32, -1]), c(255, 222, 150), emissive=True)
    for x in np.linspace(-ew / 2 + 4, ew / 2 - 4, 4):
        k.add(quad([x - 0.6, 0, -1.4], [x + 0.6, 0, -1.4], [x + 0.6, 32, -1.4], [x - 0.6, 32, -1.4]), GOLD)
    k.add(box([0, 35, -15], [ew + 18, 4, 30]), c(176, 40, 46))
    k.add(box([0, 33.4, -15], [ew + 19, 1.6, 31]), GOLD)
    for x in (-ew / 2 - 7, ew / 2 + 7):
        k.add(tube([x, 0, -28], [x, 33, -28], 2, 2, 8), st["trim"])
    bulbs(k, [-ew / 2 - 9, 33, -30.6], [ew / 2 + 9, 33, -30.6], 14)
    # the roadside sign, palms and lamps
    pylon_sign(k, HW - 10, -WD - 26, 62 if big else 48, 34, c(200, 40, 60), globe=big)
    palms(k, [(-HW + WX + 8, -WD - 10), (HW - WX - 8, -WD - 10), (-HW - 8, -WD + 10), (-HW - 8, HD * 1.6)], 86)
    for x in (-24, 24):
        lamp(k, x, -WD - 14)
    return body, base


def spa(style, rng, level=1):
    """The desert spa: a white adobe bath house with a small dome, a turquoise pool with loungers and umbrellas, palms;
    the upgrade adds a colonnade, a round hot tub and a vine-covered pergola."""
    body, base = Scene(), Scene()
    k = Kit(body, style, rng)
    st = k.st
    big = level >= 2
    # the game draws no base layer under the desert's spa (spaL1_Base is never shown), so the paving and pools are in
    # the body
    g = body
    R = 140 if big else 105
    ground_patch(g, st, -R, -R, R, R, st["pave"], y=0.6, tiles=12, rng=rng)
    W, D, H = (R * 0.55, R * 0.35, 26)
    bz = R - D - 4
    wall = c(246, 240, 228)
    # the pool in front of the house
    pool(g, -R * 0.62, -R * 0.78, R * 0.38, -R * 0.08, 0.8)
    if big:
        disc(g, R * 0.66, -R * 0.55, 12, 0.8, c(240, 236, 226))
        disc(g, R * 0.66, -R * 0.55, 9, 1.0, c(120, 210, 220))
        k.s.add_tris(ellipsoid([R * 0.66, 3, -R * 0.55], [6, 2, 6], 8, 4), c(236, 248, 250))
    k.block(-W, bz - D, W, bz + D, 0, H, wall)
    k.flat_roof(-W, bz - D, W, bz + D, H, 3.5, wall)
    k.add(tube([W * 0.35, H, bz], [W * 0.35, H + 6, bz], 10, 10, 14), wall)
    dome(k, W * 0.35, H + 6, bz, 10, c(70, 160, 170))
    k.door([-W * 0.3, 0, bz - D], (0, 0, -1), 12, 20, c(70, 150, 160))
    k.awning([-W * 0.3, 0, bz - D], (0, 0, -1), 18, y=24)
    k.windows_along((0, bz - D), (W, bz - D), 13, (0, 0, -1), 2, w=12, h=12)
    k.windows_along((-W, bz - D), (-W, bz + D), 13, (-1, 0, 0), 2, w=12, h=12)
    k.windows_along((W, bz - D), (W, bz + D), 13, (1, 0, 0), 2, w=12, h=12)
    # a colonnade along the pool side
    for x in np.linspace(-W, W, 8 if big else 6):
        k.add(tube([x, 0, bz - D - 10], [x, 22, bz - D - 10], 1.6, 1.6, 8), wall)
    k.add(box([0, 23, bz - D - 6], [W * 2 + 6, 3, 12]), wall)
    k.add(box([0, 24.6, bz - D - 6], [W * 2 + 7, 0.8, 13]), c(70, 160, 170))
    if big:
        # a pergola with vines
        px, pz = -R * 0.72, R * 0.1
        for x in (px - 12, px + 12):
            for z in (pz - 14, pz + 14):
                k.add(tube([x, 0, z], [x, 22, z], 1.2, 1.2, 6), c(170, 130, 90))
        for z in np.linspace(pz - 16, pz + 16, 7):
            k.add(box([px, 23, z], [30, 1.6, 2]), c(170, 130, 90))
        for _ in range(9):
            k.s.add_ball([px + rng.uniform(-14, 14), 24, pz + rng.uniform(-15, 15)], 4.5, c(90, 140, 60))
    for i, x in enumerate(np.linspace(-R * 0.58, R * 0.32, 7 if big else 5)):
        lounger(k, x, -R * 0.9 + 2, 0, st["aw1"] if i % 2 == 0 else c(240, 150, 120))
    for i, (x, z) in enumerate(((R * 0.6, -R * 0.2 if big else -R * 0.45), (-R * 0.8, -R * 0.3), (R * 0.62, -R * 0.82))):
        k.umbrella(x, z, 9, 18, col=st["aw1"] if i % 2 == 0 else c(240, 150, 120))
    # a palm-shaded garden with a fountain in the free corner
    disc(g, -R * 0.62, R * 0.5, R * 0.22, 0.7, c(110, 160, 80), 18)
    k.add(tube([-R * 0.62, 0, R * 0.5], [-R * 0.62, 6, R * 0.5], 5, 4, 10), c(236, 228, 214))
    k.s.add_tris(ellipsoid([-R * 0.62, 8, R * 0.5], [2, 3.5, 2], 8, 5), c(214, 240, 255), emissive=True)
    palms(k, [(-R + 8, bz - 8), (R - 8, -R + 10), (-R * 0.78, R * 0.75), (R - 10, bz + 6), (-R + 10, -R + 10)][: 5 if big else 4], 74)
    return body, base


def slide(k, pts, r=3.4, col=None):
    """A water slide: a chain of open tube pieces through the points."""
    col = c(250, 196, 40) if col is None else col
    for a, b in zip(pts[:-1], pts[1:]):
        k.add(tube(a, b, r, r, 10, caps=False), col)


def swim_club(style, rng, level=1):
    """The tropical swim club: a lagoon pool with a water slide off a little tower, a thatched cabana bar, loungers and
    umbrellas under palms; the upgrade is bigger, with a spiral slide and a diving board."""
    body, base = Scene(), Scene()
    k = Kit(body, style, rng)
    st = k.st
    big = level >= 2
    R = 115 if big else 87
    ground_patch(body, st, -R, -R, R, R, c(236, 222, 190), y=0.5, tiles=14, rng=rng)
    lagoon(body, -R * 0.12, -R * 0.15, R * 0.62, 0.8, 0.85, 0.7)
    # the cabana bar at the back
    cx, cz = R * 0.45, R * 0.62
    k.block(cx - 16, cz - 11, cx + 16, cz + 11, 0, 14, c(150, 104, 64))
    k.add(box([cx, 15, cz - 12], [36, 2, 6]), c(196, 150, 96))
    for x in np.linspace(cx - 12, cx + 12, 4):
        k.add(tube([x, 0, cz - 18], [x, 9, cz - 18], 1.2, 1.2, 6), c(120, 84, 52))
        k.add(tube([x, 9, cz - 18], [x, 10, cz - 18], 2.6, 2.6, 8), c(220, 80, 60))
    k.hip_roof(cx - 18, cz - 13, cx + 18, cz + 13, 22, 16, over=6, col=st["roof"])
    for x in (cx - 16, cx + 16):
        for z in (cz - 11, cz + 11):
            k.add(tube([x, 0, z], [x, 22, z], 1.4, 1.4, 6), c(120, 84, 52))
    # the slide tower and its slide into the pool
    tx, tz = -R * 0.72, R * 0.55
    th = 46 if big else 30
    for x in (tx - 6, tx + 6):
        for z in (tz - 6, tz + 6):
            k.add(tube([x, 0, z], [x, th, z], 1.2, 1.2, 6), c(240, 240, 236))
    k.add(box([tx, th, tz], [16, 2, 16]), c(56, 160, 190))
    k.add(np.array([[[tx - 9, th + 2, tz - 9], [tx + 9, th + 2, tz - 9], [tx, th + 12, tz]],
                    [[tx + 9, th + 2, tz - 9], [tx + 9, th + 2, tz + 9], [tx, th + 12, tz]],
                    [[tx + 9, th + 2, tz + 9], [tx - 9, th + 2, tz + 9], [tx, th + 12, tz]],
                    [[tx - 9, th + 2, tz + 9], [tx - 9, th + 2, tz - 9], [tx, th + 12, tz]]]), c(236, 90, 70))
    for y in np.arange(4, th, 6):
        k.add(box([tx + 6.5, y, tz], [1, 1, 10]), c(240, 240, 236))
    if big:
        pts = []
        for t in np.linspace(0, 1, 22):
            a = t * 2.6 * np.pi
            rad = 16
            pts.append(np.array([tx + 8 + np.cos(a) * rad, th - t * (th - 3), tz - 14 - np.sin(a) * rad * 0.8 - t * 26]))
        slide(k, pts, 3.6)
        # a diving board at the deep end
        k.add(box([R * 0.2, 4, -R * 0.66], [6, 8, 6]), c(240, 240, 236))
        k.add(box([R * 0.2, 8.5, -R * 0.52], [5, 1, 26]), c(56, 160, 190))
    else:
        pts = [np.array([tx, th, tz - 8]) + np.array([np.sin(t * 2.2) * 8, -t * (th - 3), -t * 34]) for t in np.linspace(0, 1, 12)]
        slide(k, pts, 3.2)
    cols = [st["aw1"], c(80, 180, 200), c(250, 210, 80)]
    for i, x in enumerate(np.linspace(-R * 0.78, R * 0.62, 7 if big else 6)):
        lounger(k, x, -R * 0.9 + 4, 0, cols[i % 3])
    for i, (x, z) in enumerate([(R * 0.72, -R * 0.2), (-R * 0.84, -R * 0.4), (R * 0.75, R * 0.2)][: 3 if big else 2]):
        k.umbrella(x, z, 9, 18, col=cols[i % 3])
    palms(k, [(-R + 4, -R + 6), (R - 4, -R + 8), (R - 2, R * 0.2), (-R * 0.2, R - 4)][: 4 if big else 3], 80)
    return body, base


def carousel(k, x, z, r=18.0):
    """A merry-go-round: a round platform, a striped cone roof on a pole, horses on poles round the edge."""
    k.add(tube([x, 0, z], [x, 3, z], r, r, 20), c(240, 232, 214))
    k.add(tube([x, 3, z], [x, 26, z], 2, 2, 8), GOLD)
    n = 12
    for i in range(n):
        a0, a1 = 2 * np.pi * i / n, 2 * np.pi * (i + 1) / n
        col = c(220, 60, 60) if i % 2 == 0 else c(250, 246, 236)
        k.add(np.array([[[x, 36, z], [x + np.cos(a0) * (r + 2), 24, z + np.sin(a0) * (r + 2)],
                         [x + np.cos(a1) * (r + 2), 24, z + np.sin(a1) * (r + 2)]]]), col)
        k.add(quad([x + np.cos(a0) * (r + 2), 24, z + np.sin(a0) * (r + 2)], [x + np.cos(a1) * (r + 2), 24, z + np.sin(a1) * (r + 2)],
                   [x + np.cos(a1) * (r + 2), 21, z + np.sin(a1) * (r + 2)], [x + np.cos(a0) * (r + 2), 21, z + np.sin(a0) * (r + 2)]),
              GOLD if i % 2 else col)
    for i in range(8):
        a = 2 * np.pi * (i + 0.5) / 8
        px, pz = x + np.cos(a) * (r - 4), z + np.sin(a) * (r - 4)
        k.add(tube([px, 3, pz], [px, 22, pz], 0.5, 0.5, 4), GOLD)
        k.s.add_tris(ellipsoid([px, 10 + (i % 2) * 2, pz], [2, 2.4, 4.4], 8, 5), [c(250, 250, 246), c(120, 80, 50)][i % 2])
    k.add(np.array([[[x, 40, z], [x - 1, 36, z], [x + 1, 36, z]]]), GOLD)


def ferris_wheel(k, x, z, r=34.0, axis_z=False):
    """A big wheel: two rims of spokes on an A-frame, coloured gondolas hanging round it."""
    cy = r + 8
    def P(u, v, w):
        return np.array([x + (w if axis_z else u), cy + v, z + (u if axis_z else w)])
    n = 16
    for side in (-3, 3):
        for i in range(n):
            a0, a1 = 2 * np.pi * i / n, 2 * np.pi * (i + 1) / n
            k.add(tube(P(np.cos(a0) * r, np.sin(a0) * r, side), P(np.cos(a1) * r, np.sin(a1) * r, side), 0.8, 0.8, 4), c(246, 246, 240))
            if i % 2 == 0:
                k.add(tube(P(0, 0, side), P(np.cos(a0) * r, np.sin(a0) * r, side), 0.4, 0.4, 3), c(246, 246, 240))
        k.add(tube(P(0, 0, side * 2), P(-r * 0.55, -cy, side * 3), 1.4, 1.6, 5), c(200, 204, 210))
        k.add(tube(P(0, 0, side * 2), P(r * 0.55, -cy, side * 3), 1.4, 1.6, 5), c(200, 204, 210))
    cols = [c(220, 60, 60), c(60, 120, 210), c(250, 200, 50), c(70, 170, 90)]
    for i in range(8):
        a = 2 * np.pi * i / 8
        k.add(box([0, 0, 0], [6, 6, 6]) + P(np.cos(a) * r, np.sin(a) * r - 4, 0), cols[i % 4])


def coaster(k, pts, col, r=1.2, every=3):
    """A roller coaster track: twin rails through the points on white supports to the ground (every few points)."""
    pts = [np.asarray(p, float) for p in pts]
    for a, b in zip(pts[:-1], pts[1:]):
        d = b - a
        side = np.cross(d, [0, 1, 0])
        side = side / (np.linalg.norm(side) + 1e-9) * 2.4
        k.add(tube(a + side, b + side, r, r, 5, caps=False), col)
        k.add(tube(a - side, b - side, r, r, 5, caps=False), col)
    for p in pts[::every]:
        if p[1] > 4:
            k.add(tube([p[0], 0, p[2]], p - [0, 1.5, 0], 0.9, 0.9, 4), c(236, 236, 232))


def stall(k, x, z, col):
    """A little food stall with a striped awning."""
    k.block(x - 8, z - 6, x + 8, z + 6, 0, 13, c(246, 238, 220))
    k.add(box([x, 14, z], [18, 1.5, 14]), col)
    for i in range(4):
        cc = col if i % 2 == 0 else c(250, 248, 240)
        k.add(quad([x - 9 + i * 4.5, 14, z - 7], [x - 4.5 + i * 4.5, 14, z - 7], [x - 4.5 + i * 4.5, 10, z - 12], [x - 9 + i * 4.5, 10, z - 12]), cc)


def swing_ride(k, x, z, h=46.0, r=16.0):
    """A chair swing ride: a tall striped pole with a canopy, chairs hanging out on chains."""
    k.add(tube([x, 0, z], [x, 3, z], r * 0.5, r * 0.5, 14), c(240, 232, 214))
    k.add(tube([x, 3, z], [x, h, z], 2.2, 1.8, 8), c(240, 240, 236))
    n = 10
    for i in range(n):
        a0, a1 = 2 * np.pi * i / n, 2 * np.pi * (i + 1) / n
        col = c(60, 140, 210) if i % 2 == 0 else c(250, 220, 80)
        k.add(np.array([[[x, h + 8, z], [x + np.cos(a0) * r, h, z + np.sin(a0) * r], [x + np.cos(a1) * r, h, z + np.sin(a1) * r]]]), col)
    for i in range(8):
        a = 2 * np.pi * (i + 0.5) / 8
        top = np.array([x + np.cos(a) * r * 0.9, h, z + np.sin(a) * r * 0.9])
        seat = np.array([x + np.cos(a) * r * 1.35, h - 20, z + np.sin(a) * r * 1.35])
        k.add(tube(top, seat, 0.3, 0.3, 3), c(200, 200, 200))
        k.add(box(seat - [0, 1.5, 0], [4, 2, 4]), [c(220, 60, 60), c(60, 170, 90)][i % 2])


def pirate_ship(body, x, z, size=1.0, yaw=35.0):
    """A pirate ship at anchor: a dark hull, a light deck, two masts with white sails and a black flag."""
    hull = Scene()
    hull.add_tris(box([0, 6, 0], [14, 8, 46]), c(120, 76, 44))
    hull.add_tris(box([0, 10.5, 18], [14, 3, 12]), c(100, 62, 36))
    hull.add_tris(box([0, 10.5, 0], [15, 1, 47]), c(220, 190, 110))
    for zz, hh in ((-12, 38), (6, 46)):
        hull.add_tris(tube([0, 10, zz], [0, hh, zz], 0.9, 0.7, 5), c(110, 80, 50))
        hull.add_tris(quad([-11, hh - 4, zz + 1], [11, hh - 4, zz + 1], [11, hh - 20, zz + 2], [-11, hh - 20, zz + 2]), c(248, 244, 230))
    hull.add_tris(quad([0, 46, 6], [0, 46, 14], [0, 42, 14], [0, 42, 6]), c(30, 30, 34))
    a = np.radians(yaw)
    rot = np.array([[np.cos(a), 0, np.sin(a)], [0, 1, 0], [-np.sin(a), 0, np.cos(a)]])
    body.extend(hull.transformed(size, rot=rot, move=[x, 0, z]))


def theme_park(style, rng, level=1):
    """The tropical theme park: a thatched entrance gate, paths between lawns, a lagoon with a pirate ship, a carousel, a
    swing ride, food stalls and palms; the upgrade adds a big wheel and a roller coaster with a loop."""
    body, base = Scene(), Scene()
    k = Kit(body, style, rng)
    st = k.st
    big = level >= 2
    R = 190 if big else 141
    # lawns, with light paths between them
    ground_patch(body, st, -R, -R, R, R, c(232, 220, 186), y=0.5, tiles=24, rng=rng)
    for (x0, z0, x1, z1) in ((-R + 8, -R * 0.42, -14, -14), (14, -R * 0.42, R - 8, -14), (-R + 8, 14, -14, R - 8), (14, 14, R - 8, R - 8)):
        body.add_tris(quad([x0, 0.6, z0], [x1, 0.6, z0], [x1, 0.6, z1], [x0, 0.6, z1]), c(104, 176, 80))
        body.add_tris(quad([x0 - 1.5, 0.55, z0 - 1.5], [x1 + 1.5, 0.55, z0 - 1.5], [x1 + 1.5, 0.55, z1 + 1.5], [x0 - 1.5, 0.55, z1 + 1.5]),
                      c(200, 186, 150))
    # the entrance gate at the front
    gz = -R + 6
    for x in (-24, 24):
        k.add(box([x, 20, gz], [9, 40, 9]), c(40, 140, 140))
        k.add(box([x, 41, gz], [11, 2, 11]), c(250, 210, 80))
    k.add(box([0, 44, gz], [60, 8, 11]), c(196, 160, 90))
    k.hip_roof(-32, gz - 8, 32, gz + 8, 48, 12, over=3, col=st["roof"])
    k.s.add_tris(quad([-18, 41, gz - 5.8], [18, 41, gz - 5.8], [18, 47, gz - 5.8], [-18, 47, gz - 5.8]), c(250, 220, 120), emissive=True)
    bulbs(k, [-30, 39, gz - 6], [30, 39, gz - 6], 14)
    for x in (-40, 40):
        k.add(box([x, 7, gz - 2], [16, 14, 10]), c(246, 238, 220))
        k.add(box([x, 15, gz - 2], [18, 2, 12]), c(220, 60, 60))
    # back left: the lagoon and the pirate ship
    lx, lz = -R * 0.52, R * 0.52
    lagoon(body, lx, lz, R * 0.3, 0.7, 0.8)
    pirate_ship(body, lx, lz, 1.4)
    # front left: the carousel and food stalls; front right: the swing ride
    carousel(k, -R * 0.58, -R * 0.66, 24)
    swing_ride(k, R * 0.6, -R * 0.62, 56, 18)
    for i, (x, z) in enumerate(((-R * 0.2, -R * 0.62), (-R * 0.86, -R * 0.24), (R * 0.22, -R * 0.74), (R * 0.86, -R * 0.24),
                                (-R * 0.3, -R * 0.24))):
        stall(k, x, z, [c(220, 60, 60), c(60, 150, 210), c(240, 170, 40)][i % 3])
    for i, x in enumerate((-R * 0.5, -R * 0.36, R * 0.36, R * 0.5)):
        k.umbrella(x, -R * 0.3, 8, 16, col=[c(220, 60, 60), c(60, 150, 210)][i % 2])
    # flower beds on the lawns
    for (x, z) in ((-R * 0.25, -R * 0.82), (R * 0.82, -R * 0.82), (-R * 0.86, R * 0.2), (R * 0.2, R * 0.88)):
        disc(body, x, z, 9, 0.8, c(96, 70, 50), 14)
        for a in np.linspace(0, 2 * np.pi, 7)[:-1]:
            body.add_tris(ellipsoid([x + np.cos(a) * 5, 1.8, z + np.sin(a) * 5], [3, 2, 3], 6, 4),
                          [c(232, 70, 120), c(250, 200, 70), c(240, 120, 60)][int(a * 2) % 3])
    qx, qz = R * 0.55, R * 0.55
    if big:
        # back right: a roller coaster round the lawn, up a lift hill, down a drop and round a loop
        A = R * 0.36
        pts = []
        n = 48
        for i in range(n + 1):
            t = i / n
            ang = 2 * np.pi * t
            x, z = qx + A * np.cos(ang), qz + A * 0.85 * np.sin(ang)
            if t < 0.3:
                y = 6 + t / 0.3 * 60
            elif t < 0.42:
                y = 66 - (t - 0.3) / 0.12 * 58
            else:
                y = 8 + 10 * np.sin((t - 0.42) / 0.58 * 3 * np.pi) ** 2
            pts.append(np.array([x, y, z]))
            if i == 30:
                # the loop, standing on the track's direction
                d = np.array([-np.sin(ang), 0, 0.85 * np.cos(ang)])
                d /= np.linalg.norm(d)
                side = np.cross(d, [0, 1, 0])
                lr = 24
                p0 = pts[-1]
                for th in np.linspace(0, 2 * np.pi, 20)[1:]:
                    pts.append(p0 + np.array([0, lr, 0]) + lr * (-np.cos(th) * np.array([0, 1, 0]) + np.sin(th) * d)
                               + side * th / (2 * np.pi) * 7)
        coaster(k, pts, c(200, 40, 50), r=1.5, every=2)
        # the station
        k.block(qx + A - 10, qz - 14, qx + A + 10, qz + 14, 0, 12, c(246, 238, 220))
        k.hip_roof(qx + A - 12, qz - 16, qx + A + 12, qz + 16, 14, 8, over=2, col=st["roof"])
        # front right, by the swing ride: the big wheel
        ferris_wheel(k, R * 0.3, -R * 0.2, 46)
    else:
        ferris_wheel(k, qx, qz, 38)
        stall(k, qx - R * 0.3, qz + R * 0.2, c(240, 170, 40))
    palms(k, [(-R + 10, -R + 14), (R - 10, -R + 14), (-R + 8, 0), (R - 8, 0), (-R * 0.7, R - 12), (R * 0.1, R - 10),
              (-60, gz + 6), (60, gz + 6)], 86)
    return body, base


B = lambda f, *a, **kw: (lambda st, rng: f(st, rng, *a, **kw))

FAMILIES = [
    # (regex on the lower-case path, builder(style, rng) -> (body, base))
    (r"/park/clubl1|/desert/desclubl1|/tropical/tropclubl1|/links/clubl1", lambda st, rng: clubhouse(st, 1, rng), 1.4),
    (r"/park/clubl2|/desert/desclubl2|/tropical/tropclubl2|/links/clubl2", lambda st, rng: clubhouse(st, 2, rng), 1.4),
    (r"/park/prosl1|/park/proshop|/desert/dprol1|/tropical/tropproshopl1|/links/pro_shop", B(shop, "pro", 1), 1.3),
    (r"/park/prosl2|/desert/desprol2|/tropical/tropproshopl2|/links/links_proshopl2", B(shop, "pro", 2), 1.3),
    (r"/park/parksnackl1|/desert/dessnack(base)?\.|/tropical/tropsnackl1", B(shop, "snack", 1), 1.3),
    (r"/park/parksnackl2|/desert/dessnackl2|/tropical/tropsnackl2", B(shop, "snack", 2), 1.3),
    (r"/park/cartl1|/desert/descartl1|/tropical/tropcartl1|/links/cart_garagel1", B(shop, "garage", 1), 1.2),
    (r"/park/cartl2|/desert/descartl2|/tropical/tropcartl2|/links/cart_garagel2", B(shop, "garage", 2), 1.2),
    (r"/park/builddrl1|/desert/dbuilddrl1|/tropical/tropdrivingrshopl1|/links/dr_shop", B(shop, "range", 1)),
    (r"/park/builddrl2|/desert/desdrl2house|/tropical/tropdrivingrshopl2|/links/drl2_shop", B(shop, "range", 2)),
    (r"/park/hotell1|/desert/deshotell1|/tropical/trophotell1|/links/hotell1", B(hotel, 1), 1.4),
    (r"/park/hotell2|/desert/deshol2|/tropical/trophotell2|/links/hotell2", B(hotel, 2), 1.4),
    (r"/park/puttl1|/desert/dputtl1|/tropical/tropputtinggl1|/links/pg_hutl1", B(putting, 1)),
    (r"/park/puttl2|/desert/desputtl2\.|/tropical/tropputtinggl2(?!_scenic)|/links/pg_hutl2", B(putting, 2)),
    (r"/park/tenl1|/desert/tenl1", B(tennis, 1)),
    (r"/park/tenl2|/desert/tenl2", B(tennis, 2)),
    (r"/park/houseconst|/desert/deshouseconst|/tropical/trophouseconst|/links/houseconst_links", lambda st, rng: construction(st, rng)),
    (r"/park/house_a|/desert/dhousea|/tropical/trophousea|/links/house_a_links", B(house, 0), 1.3),
    (r"/park/houseb|/tropical/trophouseb", B(house, 1), 1.3),
    (r"/links/castlel1", B(landmark_castle, 1)),
    (r"/links/castlel2", B(landmark_castle, 2)),
    (r"/links/churchl1", B(church, 1)),
    (r"/links/churchl2", B(church, 2)),
    (r"/links/stablel1", B(stable, 1)),
    (r"/links/stablel2", B(stable, 2)),
    (r"/links/pub_sq", B(pub, 1)),
    (r"/links/publ2", B(pub, 2)),
    (r"/park/marl1|/tropical/tropmarl1", B(marina, 1)),
    (r"/park/marl2|/tropical/tropmarinal2", B(marina, 2)),
    (r"/park/airl1|/desert/helil1", B(airstrip, 1)),
    (r"/park/airl2|/desert/hell2", B(airstrip, 2)),
    (r"/park/plat_topdrl|/desert/dplatdrl1\.|/tropical/tropdrivingrl.plattop|/links/drl2_platform\.|/links/dr_platform\.",
     B(range_platform, True)),
    (r"/park/plat_basedrl|/desert/dplatdrl1base|/tropical/tropdrivingrl.platbase|/links/drl2_platform_base",
     B(range_platform, False)),
    (r"/park/100drl1|/desert/d100drl1|/tropical/tropdrivingr100l1|/links/dr_100sign", B(distance_sign, 100)),
    (r"/park/200drl1|/desert/d200drl1|/tropical/tropdrivingr200l1|/links/dr_200sign", B(distance_sign, 200)),
    (r"/park/300drl1|/desert/d300drl1|/tropical/tropdrivingr300l1|/links/dr_300sign", B(distance_sign, 300)),
    (r"/desert/desdrl2400|/tropical/tropdrivingr400l2|/links/dr_400sign", B(distance_sign, 400)),
    (r"/desert/descasl1(base)?\.", B(casino, 1), 1.75),
    (r"/desert/descasl2(base)?\.", B(casino, 2), 1.75),
    (r"/desert/spal1(_base)?\.", B(spa, 1), 1.4),
    (r"/desert/spal2\.", B(spa, 2), 1.4),
    (r"/tropical/tropswiml1\.", B(swim_club, 1), 1.4),
    (r"/tropical/tropswiml2\.", B(swim_club, 2), 1.4),
    (r"/tropical/tropthemel1\.", B(theme_park, 1), 1.55),
    (r"/tropical/tropthemel2\.", B(theme_park, 2), 1.55),
]
