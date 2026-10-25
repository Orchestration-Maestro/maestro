use maestro_terminal::input::mouse_encode::size::{
    CellSize, Coordinate, GridSize, Padding, ScreenSize, Size, Space,
};

fn size() -> Size {
    Size {
        screen: ScreenSize {
            width: 100,
            height: 100,
        },
        cell: CellSize {
            width: 5,
            height: 10,
        },
        padding: Padding::default(),
    }
}

#[test]
fn grid_size_update_exact() {
    let mut grid = GridSize::default();
    grid.update(
        ScreenSize {
            width: 100,
            height: 40,
        },
        CellSize {
            width: 5,
            height: 10,
        },
    );
    assert_eq!(
        grid,
        GridSize {
            columns: 20,
            rows: 4
        }
    );
}

#[test]
fn grid_size_update_rounding() {
    let mut grid = GridSize::default();
    grid.update(
        ScreenSize {
            width: 20,
            height: 40,
        },
        CellSize {
            width: 6,
            height: 15,
        },
    );
    assert_eq!(
        grid,
        GridSize {
            columns: 3,
            rows: 2
        }
    );
}

#[test]
fn coordinate_conversion() {
    for (x, y, col, row) in [
        (0.0, 0.0, 0, 0),
        (6.0, 0.0, 1, 0),
        (6.0, 10.0, 1, 1),
        (-10.0, -10.0, 0, 0),
        (100_000.0, 100_000.0, 19, 9),
    ] {
        assert_eq!(
            Coordinate::Surface { x, y }.convert(Space::Grid, size()),
            Coordinate::Grid { x: col, y: row }
        );
    }
}

#[test]
fn padding_and_coordinate_spaces() {
    let size = Size {
        padding: Padding {
            left: 7,
            right: 3,
            top: 9,
            bottom: 1,
        },
        ..size()
    };
    assert_eq!(
        size.terminal(),
        ScreenSize {
            width: 90,
            height: 90
        }
    );
    assert_eq!(
        size.grid(),
        GridSize {
            columns: 18,
            rows: 9
        }
    );
    let surface = Coordinate::Surface { x: 17.0, y: 29.0 };
    let terminal = Coordinate::Terminal { x: 10.0, y: 20.0 };
    let grid = Coordinate::Grid { x: 2, y: 2 };
    for coordinate in [surface, terminal, grid] {
        assert_eq!(coordinate.convert(Space::Surface, size), surface);
        assert_eq!(coordinate.convert(Space::Terminal, size), terminal);
        assert_eq!(coordinate.convert(Space::Grid, size), grid);
    }
    assert_eq!(
        size.screen
            .blank_padding(size.padding, size.grid(), size.cell),
        Padding::default()
    );
}

#[test]
fn undersized_surface_retains_one_cell() {
    let size = Size {
        screen: ScreenSize {
            width: 1,
            height: 1,
        },
        padding: Padding {
            left: 10,
            right: 10,
            top: 10,
            bottom: 10,
        },
        ..size()
    };
    assert_eq!(
        size.terminal(),
        ScreenSize {
            width: 0,
            height: 0
        }
    );
    assert_eq!(
        size.grid(),
        GridSize {
            columns: 1,
            rows: 1
        }
    );
    assert_eq!(
        size.screen
            .blank_padding(size.padding, size.grid(), size.cell),
        Padding::default()
    );
}
