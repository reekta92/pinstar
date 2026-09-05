//! Viewport math shared by pan/zoom/navigation (ported from clin's
//! `ui::camera`, originally salvaged from graf's viewport module).

pub const CANVAS_ZOOM_MIN: f64 = 0.05;

pub enum ZoomDir {
    In,
    Out,
}

pub fn clamp_world(v: f64) -> f64 {
    const WORLD_COORD_LIMIT: f64 = 1.0e18;
    if !v.is_finite() {
        return 0.0;
    }
    v.clamp(-WORLD_COORD_LIMIT, WORLD_COORD_LIMIT)
}

/// Returns None on non-finite → caller leaves center unchanged.
pub fn pan_centered(cx: f64, cy: f64, dx: f64, dy: f64) -> Option<(f64, f64)> {
    if !dx.is_finite() || !dy.is_finite() {
        return None;
    }
    let (nx, ny) = (cx + dx, cy + dy);
    if nx.is_finite() && ny.is_finite() {
        Some((nx, ny))
    } else {
        None
    }
}

/// In: zoom*factor. Out: zoom/factor floored at `min`. Rejects non-finite.
pub fn zoom_step(zoom: f64, factor: f64, dir: ZoomDir, min: f64) -> Option<f64> {
    let next = match dir {
        ZoomDir::In => zoom * factor,
        ZoomDir::Out => zoom / factor,
    };
    if !next.is_finite() {
        return None;
    }
    Some(next.max(min))
}

/// 60° cone forward search. `cands` = candidate positions in view iteration
/// order (caller excludes the current node BEFORE building the slice so ties
/// resolve identically). Returns index into `cands`.
pub fn nearest_in_dir(
    cands: &[(f64, f64)],
    origin: (f64, f64),
    dir: (f64, f64),
    cone: f64,
) -> Option<usize> {
    let dir_len = (dir.0 * dir.0 + dir.1 * dir.1).sqrt();
    if dir_len == 0.0 {
        return None;
    }
    let dir = (dir.0 / dir_len, dir.1 / dir_len);

    let mut best: Option<(usize, f64)> = None;
    for (i, &(x, y)) in cands.iter().enumerate() {
        let vx = x - origin.0;
        let vy = y - origin.1;
        let dist = (vx * vx + vy * vy).sqrt();
        if dist < 1e-9 {
            continue;
        }
        let cos = (vx * dir.0 + vy * dir.1) / dist;
        let angle = cos.acos();
        if angle > cone {
            continue;
        }
        // Score: distance + angular penalty so straighter wins on ties.
        let score = dist * (1.0 + angle);
        match best {
            Some((_, best_score)) if score >= best_score => {}
            _ => best = Some((i, score)),
        }
    }
    best.map(|(i, _)| i)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zoom_in_out_bounds() {
        assert!(zoom_step(1.0, 1.1, ZoomDir::In, 0.0).unwrap() > 1.0);
        assert!(zoom_step(1.0, 1.1, ZoomDir::Out, 0.05).unwrap() < 1.0);
        assert_eq!(zoom_step(0.04, 1.1, ZoomDir::Out, 0.05).unwrap(), 0.05);
        assert!(zoom_step(f64::NAN, 1.1, ZoomDir::In, 0.0).is_none());
    }

    #[test]
    fn clamp_world_nonfinite() {
        assert_eq!(clamp_world(f64::NAN), 0.0);
        assert_eq!(clamp_world(5.0), 5.0);
    }

    #[test]
    fn nearest_in_cone_prefers_straightest() {
        let cands = [(1.0, 0.0), (1.0, 0.4)];
        let idx = nearest_in_dir(&cands, (0.0, 0.0), (1.0, 0.0), std::f64::consts::FRAC_PI_3);
        assert_eq!(idx, Some(0));
    }
}
