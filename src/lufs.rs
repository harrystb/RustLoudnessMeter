use std::collections::VecDeque;

struct KFilterStage {
    a1: f32,
    a2: f32,
    b0: f32,
    b1: f32,
    b2: f32,
    prev: f32,
    prev2: f32,
}

impl KFilterStage {
    fn new_stage_1() -> KFilterStage {
        KFilterStage {
            a1: -1.69065929318241,
            a2:  0.73248077421585,
            b0:  1.53512485958697,
            b1: -2.69169618940638,
            b2:  1.19839281085285,
            prev: 0.0,
            prev2: 0.0,
        }
    }
    fn new_stage_2() -> KFilterStage {
        KFilterStage {
            a1: -1.99004745483398,
            a2:  0.99007225036621,
            b0:  1.0,
            b1: -2.0,
            b2:  1.0,
            prev: 0.0,
            prev2: 0.0,
        }
    }

    fn next(&mut self, val: f32) -> f32 {
        let z = val - self.a1 * self.prev - self.a2 * self.prev2;
        let out = z * self.b0 + self.prev * self.b1 + self.prev2 * self.b2;
        self.prev2 = self.prev;
        self.prev = z;
        out

    }
}

struct KFilter {
    stage1: KFilterStage,
    stage2: KFilterStage,
}

impl KFilter {
    fn new() -> KFilter {
        KFilter {
            stage1: KFilterStage.new_stage_1(),
            stage2: KFilterStage.new_stage_2(),
        }
    }

    fn next(&mut self, val: f32) -> f32 {
        self.stage2.next(self.stage1.next(val))
    }
}

struct LUFSCalculator {
    filter: KFilter,
    filtered_buf: VecDeque<f32>;
}

impl LUFSCalculator {
    fn new() -> LUFSCalculator {
        LUFSCalculator {
            filter: KFilter::new(),
            filtered_buf: VecDeque::with_capacity(19200), //400ms with 48kHz -> 0.4 * 48000 =
        }
    }
}