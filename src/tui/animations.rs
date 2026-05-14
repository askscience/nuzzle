use std::time::Instant;

pub struct BrailleSpinner {
    frames: &'static [&'static str],
    frame: usize,
    start: Instant,
}

impl Default for BrailleSpinner {
    fn default() -> Self {
        Self {
            frames: &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"],
            frame: 0,
            start: Instant::now(),
        }
    }
}

impl BrailleSpinner {
    pub fn update(&mut self) {
        if self.start.elapsed().as_millis() >= 80 {
            self.frame = (self.frame + 1) % self.frames.len();
            self.start = Instant::now();
        }
    }

    pub fn current(&self) -> &'static str {
        self.frames[self.frame]
    }
}

pub struct BraillePulse {
    frames: &'static [&'static str],
    frame: usize,
    start: Instant,
}

impl Default for BraillePulse {
    fn default() -> Self {
        Self {
            frames: &["⣀", "⣤", "⣶", "⣿", "⣶", "⣤", "⣀"],
            frame: 0,
            start: Instant::now(),
        }
    }
}

impl BraillePulse {
    pub fn update(&mut self) {
        if self.start.elapsed().as_millis() >= 120 {
            self.frame = (self.frame + 1) % self.frames.len();
            self.start = Instant::now();
        }
    }

    pub fn current(&self) -> &'static str {
        self.frames[self.frame]
    }
}

pub struct BrailleProgress {
    frames: &'static [&'static str],
    frame: usize,
    start: Instant,
}

impl Default for BrailleProgress {
    fn default() -> Self {
        Self {
            frames: &["⡀", "⡄", "⡆", "⡇", "⡆", "⡄", "⡀"],
            frame: 0,
            start: Instant::now(),
        }
    }
}

impl BrailleProgress {
    pub fn update(&mut self) {
        if self.start.elapsed().as_millis() >= 100 {
            self.frame = (self.frame + 1) % self.frames.len();
            self.start = Instant::now();
        }
    }

    pub fn current(&self) -> &'static str {
        self.frames[self.frame]
    }
}
