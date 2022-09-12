use std::collections::VecDeque;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::thread;

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
            stage1: KFilterStage::new_stage_1(),
            stage2: KFilterStage::new_stage_2(),
        }
    }

    fn next(&mut self, val: f32) -> f32 {
        self.stage2.next(self.stage1.next(val))
    }
}

pub struct LUFSCalculator {
    filter: KFilter,
    filtered_buf: VecDeque<f32>,
    rx_chan: Receiver<f32>,
    tx_chan: Sender<f32>,
}

impl LUFSCalculator {
    pub fn start(rx_chan: Receiver<f32>, tx_chan: Sender<f32>) {
        thread::spawn(move|| {
            let mut calc = LUFSCalculator {
                filter: KFilter::new(),
                filtered_buf: VecDeque::with_capacity(19200), //400ms with 48kHz -> 0.4 * 48000 = 19200
                rx_chan,
                tx_chan,
            };

            while let Ok(val) = calc.rx_chan.recv() {
                calc.filtered_buf.push_back(calc.filter.next(val));
                if calc.filtered_buf.len() == 19200 {
                    // calculate and clear 25% of the buffer
                    match calc.tx_chan.send(-0.691 + 10.*(calc.filtered_buf.iter().map(|x| x*x).sum::<f32>()/19200.).log10()) {
                        Ok(_) => (),
                        Err(e) => break,
                    }
                    for _ in 0..1200 { // was 0..4800
                        calc.filtered_buf.pop_front();
                    }
                }
            }
        });
    }
}



#[cfg(test)]
mod tests {
    use std::f32::consts::PI;
    use super::*;

    #[test]
    fn test_1kHz() {
        let (tx_chan, rx_chan) = channel();
        let (tx2_chan, rx2_chan) = channel();

        LUFSCalculator::start(rx_chan, tx2_chan);
        for i in 0..19200 {
            tx_chan.send(((i as f32) * 2.0 * PI * 997.0 / 48000.).sin()).expect("should not happen");
        }
        match rx2_chan.recv() {
            Ok(v) => assert!(v > -3.02 && v < -3.0), // should be -3.01
            Err(e) => panic!("Should not happen"),
        }
    }

}