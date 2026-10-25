//! Load-time work spread over threads: independent items (the frames of a
//! motion, the clips of a kit) computed on up to eight threads and returned
//! in their order, so the result is the same as a plain loop's.

/// `f(0..n)` on several threads, in order. The first error (by index) is
/// returned, as a loop would have returned it.
pub fn map_indexed<T, E, F>(n: usize, f: F) -> Result<Vec<T>, E>
where
    T: Send,
    E: Send,
    F: Fn(usize) -> Result<T, E> + Sync,
{
    let threads = std::thread::available_parallelism()
        .map_or(1, |t| t.get())
        .clamp(1, 8)
        .min(n.max(1));
    if threads <= 1 || n < 4 {
        return (0..n).map(f).collect();
    }
    let f = &f;
    let mut parts: Vec<(usize, Result<T, E>)> = std::thread::scope(|scope| {
        let workers: Vec<_> = (0..threads)
            .map(|t| {
                scope.spawn(move || {
                    (t..n)
                        .step_by(threads)
                        .map(|i| (i, f(i)))
                        .collect::<Vec<_>>()
                })
            })
            .collect();
        workers
            .into_iter()
            .flat_map(|w| w.join().expect("load worker"))
            .collect()
    });
    parts.sort_by_key(|(i, _)| *i);
    parts.into_iter().map(|(_, r)| r).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_order_and_the_first_error() {
        let squares: Result<Vec<usize>, ()> = map_indexed(100, |i| Ok(i * i));
        assert_eq!(
            squares.unwrap(),
            (0..100).map(|i| i * i).collect::<Vec<_>>()
        );
        let failed: Result<Vec<usize>, usize> =
            map_indexed(50, |i| if i % 7 == 3 { Err(i) } else { Ok(i) });
        assert_eq!(failed, Err(3));
        let none: Result<Vec<u8>, ()> = map_indexed(0, |_| Ok(0));
        assert!(none.unwrap().is_empty());
    }
}
