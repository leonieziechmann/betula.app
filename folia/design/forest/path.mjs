// Path data as short as it gets: every command relative to where the pen is, a command letter only
// where it changes (after a move the pen draws lines on its own), and no separator a minus sign or
// the letter already makes. The shape stays the same, point for point; gzip and brotli find the
// small relative numbers again and again. Even the first move is relative (to the origin, where
// the pen starts): after an absolute M the pairs that follow would be absolute lines.
const fmt = (v) => String(Math.round(v * 1000) / 1000 || 0);
const join = (nums) => nums.map((n, i) => (i && !n.startsWith("-") ? " " : "") + n).join("");

export function compact(d) {
  const tokens = d.match(/[MLQZmlqzAa]|-?\d*\.?\d+(?:e-?\d+)?/g) || [];
  let i = 0, x = 0, y = 0, sx = 0, sy = 0, out = "", last = "";
  const num = () => Number(tokens[i++]);
  const emit = (cmd, nums, implicit) => {
    const body = join(nums.map(fmt));
    if (cmd === last || cmd === implicit) out += (/^-/.test(body) ? "" : " ") + body;
    else out += cmd + body;
    last = cmd === "m" ? "l" : cmd;
  };
  let cmd = "";
  while (i < tokens.length) {
    if (/[A-Za-z]/.test(tokens[i])) cmd = tokens[i++];
    switch (cmd) {
      case "M": { const nx = num(), ny = num(); out += "m" + join([fmt(nx - x), fmt(ny - y)]); x = sx = nx; y = sy = ny; last = "l"; cmd = "L"; break; }
      case "L": { const nx = num(), ny = num(); emit("l", [nx - x, ny - y]); x = nx; y = ny; break; }
      case "Q": { const cx = num(), cy = num(), nx = num(), ny = num(); emit("q", [cx - x, cy - y, nx - x, ny - y]); x = nx; y = ny; break; }
      case "a": { const v = [num(), num(), num(), num(), num(), num(), num()]; emit("a", v); x += v[5]; y += v[6]; break; }
      case "Z": case "z": out += "z"; last = "z"; x = sx; y = sy; break;
      default: throw new Error("path: " + cmd);
    }
  }
  return out;
}
