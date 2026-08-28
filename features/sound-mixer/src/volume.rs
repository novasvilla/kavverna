/// PipeWire stores amplitude, but every mixer people know shows the cubic percentage, so a
/// slider at half reads 0.125 in the graph. Measured against `pactl` on a live sink.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Volume {
    percent: f32,
}

/// Above this the limiter cannot save badly mastered audio from clipping, and PipeWire does
/// no limiting of its own.
pub const MAX_PERCENT: f32 = 200.0;
pub const UNITY_PERCENT: f32 = 100.0;

impl Volume {
    pub fn from_percent(percent: f32) -> Self {
        Self { percent: percent.clamp(0.0, MAX_PERCENT) }
    }

    pub fn from_amplitude(amplitude: f32) -> Self {
        Self::from_percent(amplitude.max(0.0).cbrt() * 100.0)
    }

    pub fn percent(self) -> f32 {
        self.percent
    }

    pub fn amplitude(self) -> f32 {
        (self.percent / 100.0).powi(3)
    }

    /// Anything above unity is gain the source never had, so it is worth warning about.
    pub fn is_boosted(self) -> bool {
        self.percent > UNITY_PERCENT
    }
}

impl Default for Volume {
    fn default() -> Self {
        Self { percent: UNITY_PERCENT }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f32, b: f32) -> bool {
        (a - b).abs() < 0.001
    }

    /// The exact figures a live PipeWire sink reported for these slider positions.
    #[test]
    fn the_scale_matches_what_pipewire_stores() {
        assert!(close(Volume::from_percent(100.0).amplitude(), 1.0));
        assert!(close(Volume::from_percent(50.0).amplitude(), 0.125));
        assert!(close(Volume::from_percent(25.0).amplitude(), 0.015625));
    }

    #[test]
    fn reading_an_amplitude_gives_the_slider_back() {
        for percent in [0.0, 25.0, 50.0, 100.0, 150.0, 200.0] {
            let round_trip = Volume::from_amplitude(Volume::from_percent(percent).amplitude());
            assert!(
                close(round_trip.percent(), percent),
                "{percent} became {}",
                round_trip.percent()
            );
        }
    }

    #[test]
    fn the_slider_cannot_be_pushed_past_the_cap() {
        assert_eq!(Volume::from_percent(500.0).percent(), MAX_PERCENT);
        assert_eq!(Volume::from_percent(-10.0).percent(), 0.0);
    }

    #[test]
    fn boost_starts_above_unity() {
        assert!(!Volume::from_percent(100.0).is_boosted());
        assert!(Volume::from_percent(101.0).is_boosted());
    }

    #[test]
    fn silence_stays_silent() {
        assert_eq!(Volume::from_percent(0.0).amplitude(), 0.0);
        assert_eq!(Volume::from_amplitude(0.0).percent(), 0.0);
    }
}
