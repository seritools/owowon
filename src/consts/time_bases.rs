use crate::data::units::Time;

pub const TIME_BASES: [Time; 36] = {
    [
        Time(2.0e-9),
        Time(5.0e-9),
        Time(10.0e-9),
        Time(20.0e-9),
        Time(50.0e-9),
        Time(100.0e-9),
        Time(200.0e-9),
        Time(500.0e-9),
        Time(1.0e-6),
        Time(2.0e-6),
        Time(5.0e-6),
        Time(10.0e-6),
        Time(20.0e-6),
        Time(50.0e-6),
        Time(100.0e-6),
        Time(200.0e-6),
        Time(500.0e-6),
        Time(1.0e-3),
        Time(2.0e-3),
        Time(5.0e-3),
        Time(10.0e-3),
        Time(20.0e-3),
        Time(50.0e-3),
        Time(100.0e-3),
        Time(200.0e-3),
        Time(500.0e-3),
        Time(1.0),
        Time(2.0),
        Time(5.0),
        Time(10.0),
        Time(20.0),
        Time(50.0),
        Time(100.0),
        Time(200.0),
        Time(500.0),
        Time(1000.0),
    ]
};
