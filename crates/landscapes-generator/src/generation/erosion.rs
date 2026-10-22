//! Малые термо- и гидро-эрозия рельефа (детерминированно, без RNG).
//!
//! Лёгкие модели эрозии применяются к полю высот ПОСЛЕ шума и ДО расчёта
//! воды: они слегка скругляют крутые уступы, делают склоны и русла естественнее
//! и при этом не трогают «где вода» (уровень воды считается по уже
//! эродированной поверхности).
//!
//! - **Тепловая** ([`thermal`]): на склонах круче угла естественного откоса
//!   материал «осыпается» вниз — смягчаются бровки уступов и стенки русел.
//! - **Гидравлическая** ([`hydraulic`]): вода стекает по крутейшему спуску и
//!   переносит материал вниз — склоны становятся положе, русла чуть
//!   углубляются у порогов, а осыпь ложится в понижения.
//!
//! Параметры подобраны «небольшими»: рельеф остаётся структурным (уступы,
//! каньоны и каскады не «слизываются»).

/// 4-связные соседи узла; заполняет `out`, возвращает число соседей.
fn neighbors4(
    i: usize,
    j: usize,
    n: usize,
    m: usize,
    out: &mut [(usize, usize); 4],
) -> usize {
    let mut k = 0;
    if i > 0 {
        out[k] = (i - 1, j);
        k += 1;
    }
    if i + 1 < n {
        out[k] = (i + 1, j);
        k += 1;
    }
    if j > 0 {
        out[k] = (i, j - 1);
        k += 1;
    }
    if j + 1 < m {
        out[k] = (i, j + 1);
        k += 1;
    }
    k
}

/// Тепловая эрозия: `iters` проходов сползания материала со склонов, где
/// перепад с соседом больше `talus * step` (тангенс угла откоса `talus`).
/// Доля `rate` избытка переносится к соседу. Один проход читает исходник и
/// накапливает поправки, поэтому детерминирован и не зависит от порядка обхода.
pub fn thermal(
    h: &mut [f32],
    n: usize,
    m: usize,
    step: f32,
    talus: f32,
    rate: f32,
    iters: usize,
) {
    if n == 0 || m == 0 {
        return;
    }
    let mut scratch = h.to_vec();
    let mut deltas = vec![0.0f32; h.len()];
    let mut nb: [(usize, usize); 4] = [(0, 0); 4];
    let talus_h = talus * step;
    for _ in 0..iters {
        scratch.copy_from_slice(h);
        deltas.fill(0.0);
        for j in 0..m {
            for i in 0..n {
                let idx = j * n + i;
                let hi = scratch[idx];
                let k = neighbors4(i, j, n, m, &mut nb);
                for &(ni, nj) in &nb[..k] {
                    let nidx = nj * n + ni;
                    let excess = hi - scratch[nidx] - talus_h;
                    if excess > 0.0 {
                        let move_by = excess * rate;
                        deltas[idx] -= move_by;
                        deltas[nidx] += move_by;
                    }
                }
            }
        }
        for k in 0..h.len() {
            h[k] += deltas[k];
        }
    }
}

/// Гидравлическая эрозия: `iters` проходов, в каждом материал уносится с
/// крутейшего спуска (единый поток к самому низкому соседу) со склонов круче
/// `flow_slope` (в «высота/шаг») и откладывается на соседе. Мягче тепловой,
/// чтобы не «слизать» русла и каскады.
pub fn hydraulic(h: &mut [f32], n: usize, m: usize, step: f32, flow_slope: f32, iters: usize) {
    if n == 0 || m == 0 {
        return;
    }
    let mut scratch = h.to_vec();
    let mut deltas = vec![0.0f32; h.len()];
    let mut nb: [(usize, usize); 4] = [(0, 0); 4];
    let slope_h = flow_slope * step;
    for _ in 0..iters {
        scratch.copy_from_slice(h);
        deltas.fill(0.0);
        for j in 0..m {
            for i in 0..n {
                let idx = j * n + i;
                let hi = scratch[idx];
                let k = neighbors4(i, j, n, m, &mut nb);
                let mut lo_idx = idx;
                let mut lo = hi;
                for &(ni, nj) in &nb[..k] {
                    let v = scratch[nj * n + ni];
                    if v < lo {
                        lo = v;
                        lo_idx = nj * n + ni;
                    }
                }
                if lo_idx == idx {
                    continue;
                }
                let excess = hi - lo - slope_h;
                if excess > 0.0 {
                    let move_by = excess * 0.10;
                    deltas[idx] -= move_by;
                    deltas[lo_idx] += move_by;
                }
            }
        }
        for k in 0..h.len() {
            h[k] += deltas[k];
        }
    }
}

/// Небольшая термо- + гидро-эрозия (разумные значения по умолчанию).
pub fn erode(h: &mut [f32], n: usize, m: usize, step: f32) {
    // Тепловая: осыпь только на почти вертикальных склонах (angle откоса ~49°).
    thermal(h, n, m, step, 1.15, 0.12, 2);
    // Гидро: чуть «полирует» склоны круче ~16°, переносит материал вниз.
    hydraulic(h, n, m, step, 0.28, 2);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Холм: высокий центр, низкие края. Эрозия должна смягчить перепад,
    /// но не перевернуть рельеф (центр остаётся выше краёв).
    #[test]
    fn erosion_soffens_but_keeps_structure() {
        let n = 17;
        let center = (n / 2) as f32;
        let mut h = vec![0f32; n * n];
        for j in 0..n {
            for i in 0..n {
                let d = ((i as f32 - center).powi(2) + (j as f32 - center).powi(2)).sqrt();
                h[j * n + i] = 6.0 * (1.0 - d / center).max(0.0) + 1.0;
            }
        }
        let original = h.clone();
        erode(&mut h, n, n, 0.5);
        // Центр остаётся выше периферии.
        assert!(h[center as usize * n + center as usize] > h[0]);
        // Ужесточение перепада: вершина ниже, подножие выше исходного.
        assert!(h[center as usize * n + center as usize] < original[center as usize * n + center as usize]);
        let edge_mid = h[center as usize]; // середина нижней кромки
        let orig_mid = original[center as usize];
        assert!(edge_mid > orig_mid - 1e-4, "материал должен осесть к подножию");
    }

    #[test]
    fn erosion_is_deterministic_and_finite() {
        let n = 23;
        let mut a = vec![0f32; n * n];
        for j in 0..n {
            for i in 0..n {
                a[j * n + i] = (i as f32 * 0.9).sin() + (j as f32 * 0.7).cos() * 2.0 + (i * j % 7) as f32 * 0.1;
            }
        }
        let mut b = a.clone();
        erode(&mut a, n, n, 0.5);
        erode(&mut b, n, n, 0.5);
        assert_eq!(a, b);
        assert!(a.iter().all(|v| v.is_finite()));
    }
}
