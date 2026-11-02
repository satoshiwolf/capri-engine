use windows::Win32::Foundation::RECT;

#[derive(Debug, Clone)]
pub struct Ledge {
    pub hwnd: isize,
    pub title: String,
    pub left: i32,
    pub right: i32,
    pub y: i32,
    pub is_floor: bool,
}

/// Returns the exposed ledge intervals after subtracting windows higher in the Z-order.
pub fn open_spans(left: i32, right: i32, y: i32, above_rects: &[RECT]) -> Vec<(i32, i32)> {
    let mut spans = vec![(left, right)];

    for cover in above_rects {
        if y < cover.top || y >= cover.bottom {
            continue;
        }

        let mut next_spans = Vec::new();
        for (span_left, span_right) in spans {
            if cover.right <= span_left || cover.left >= span_right {
                next_spans.push((span_left, span_right));
                continue;
            }

            if cover.left > span_left {
                next_spans.push((span_left, cover.left));
            }
            if cover.right < span_right {
                next_spans.push((cover.right, span_right));
            }
        }
        spans = next_spans;
    }

    spans
}

pub fn scan_ledges(skip_hwnds: &[isize]) -> Vec<Ledge> {
    crate::desktop::hooks::scan_ledges(skip_hwnds)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_open_spans_subtraction() {
        let cover = RECT {
            left: 50,
            top: 50,
            right: 150,
            bottom: 150,
        };

        let spans = open_spans(0, 200, 100, &[cover]);

        assert_eq!(spans.len(), 2);
        assert_eq!(spans[0], (0, 50));
        assert_eq!(spans[1], (150, 200));
    }
}
