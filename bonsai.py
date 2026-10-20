# A port of cbonsai's tree growth (https://gitlab.com/jallbrit/cbonsai) for the
# Bonsai face, matching `cbonsai --live --infinite --life=16 --time=0.1`.
# cbonsai grows branches recursively; here the recursion is an explicit stack so
# Tree.step() can place one string per call (one per 0.1s in live mode).
import random

TRUNK, SHOOT_LEFT, SHOOT_RIGHT, DYING, DEAD = range(5)
# Styles step() returns, for the face to colour (cbonsai's colour pairs)
WOOD_BOLD, WOOD, LEAF_BOLD, LEAF, LEAF_BRIGHT_BOLD, LEAF_BRIGHT = range(6)

# The pot (cbonsai's base type 1): rows of (text, style) runs; None = pot colour
BASE = (
    ((':', None), ('___________', LEAF), ('./~~~\\.', WOOD_BOLD), ('___________', LEAF), (':', None)),
    ((' \\                           / ', None),),
    (('  \\_________________________/ ', None),),
    (('  (_)                     (_)', None),),
)
BASE_W, BASE_H = 31, 4


def _r(n):
    return random.getrandbits(16) % n


def _pick(dice, table):
    """table: upper bounds of each bucket, paired with the value for it."""
    for top, v in table:
        if dice <= top:
            return v


class Tree:
    def __init__(self, rows, cols, life=16, multiplier=5, leaves='&'):
        self.rows = rows
        self.life = life
        self.mult = multiplier
        self.leaves = leaves
        self.shoots = _r(1 << 16)
        # Each frame: [y, x, type, life, shoot cooldown, dx, dy, moved-yet flag]
        self.stack = [[rows - 1, cols // 2, TRUNK, life, multiplier, 0, 0, False]]

    def step(self):
        """Grow until the next string is placed: returns (y, x, text, style), or
        None once the tree is finished."""
        st = self.stack
        m = self.mult
        while st:
            f = st[-1]
            y, x, kind, life, cool = f[0], f[1], f[2], f[3], f[4]
            if f[7]:
                # Second half of an iteration, after any child branch has grown
                f[7] = False
                f[4] = cool - 1
                dx, dy = f[5], f[6]
                f[0] = y = y + dy
                f[1] = x = x + dx
                return y, x, self._string(kind, life, dx, dy), self._style(kind)
            if life <= 0:
                st.pop()
                continue
            f[3] = life = life - 1
            dx, dy = self._deltas(kind, life, self.life - life)
            if dy > 0 and y > self.rows - 2:
                dy -= 1                                  # too close to the ground
            f[5], f[6], f[7] = dx, dy, True
            child = None
            if life < 3:
                child = DEAD, life                       # near-dead: lots of leaves
            elif kind != DYING and kind != DEAD and life < m + 2:
                child = DYING, life
            elif kind == TRUNK and (_r(3) == 0 or life % m == 0):
                if _r(8) == 0 and life > 7:
                    f[4] = m * 2
                    child = TRUNK, life + _r(5) - 2     # a second trunk
                elif cool <= 0:
                    f[4] = m * 2
                    self.shoots += 1
                    child = SHOOT_LEFT + self.shoots % 2, life + m
            if child:
                st.append([y, x, child[0], child[1], m, 0, 0, False])
        return None

    def _deltas(self, kind, life, age):
        m = self.mult
        if kind == TRUNK:
            if age <= 2 or life < 4:                     # new or dead trunk
                return _r(3) - 1, 0
            if age < m * 3:                              # young trunk grows wide
                dy = -1 if age % int(m * 0.5) == 0 else 0
                return _pick(_r(10), ((0, -2), (3, -1), (5, 0), (8, 1), (9, 2))), dy
            return _r(3) - 1, (-1 if _r(10) > 2 else 0)
        if kind == SHOOT_LEFT or kind == SHOOT_RIGHT:
            dy = _pick(_r(10), ((1, -1), (7, 0), (9, 1)))
            dx = _pick(_r(10), ((1, -2), (5, -1), (8, 0), (9, 1)))
            return (dx if kind == SHOOT_LEFT else -dx), dy
        if kind == DYING:
            dy = _pick(_r(10), ((1, -1), (8, 0), (9, 1)))
            dx = _pick(_r(15), ((0, -3), (2, -2), (5, -1), (8, 0), (11, 1), (13, 2), (14, 3)))
            return dx, dy
        return _r(3) - 1, _pick(_r(10), ((2, -1), (6, 0), (9, 1)))     # dead

    def _string(self, kind, life, dx, dy):
        if life < 4 or kind >= DYING:
            return self.leaves[_r(len(self.leaves))]
        if kind == TRUNK:
            if dy == 0:
                return '/~'
            return '\\|' if dx < 0 else '/|\\' if dx == 0 else '|/'
        if dy > 0:
            return '\\' if kind == SHOOT_LEFT else '/'
        if dy == 0:
            return '\\_' if kind == SHOOT_LEFT else '_/'
        return '\\|' if dx < 0 else '/|' if dx == 0 else '/'

    def _style(self, kind):
        if kind == DYING:
            return LEAF_BOLD if _r(10) == 0 else LEAF
        if kind == DEAD:
            return LEAF_BRIGHT_BOLD if _r(3) == 0 else LEAF_BRIGHT
        return WOOD_BOLD if _r(2) == 0 else WOOD
