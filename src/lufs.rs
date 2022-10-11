use std::collections::VecDeque;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::thread;
use rustfft::{FftPlanner, num_complex::Complex};
use rustfft::num_complex::ComplexFloat;

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
    pub fn start(rx_chan: Receiver<f32>, tx_chan: Sender<f32>, sample_rate: u32) {
        thread::spawn(move|| {

            if sample_rate > 48000 {
                panic!("Sample rate to high.")
            }
            println!("Starting the Calculator...");

            let mut needs_upsampling = false;

            // TODO: refactor so that these are not created when the sample rate is 48000
            let raw_buf_len = (sample_rate/10) as usize; // 100ms of samples
            let mut planner = FftPlanner::new();
            let mut fft = planner.plan_fft_forward(raw_buf_len);
            let mut ifft = planner.plan_fft_inverse(4800);
            let mut raw_buffer: Vec<Complex<f32>> = Vec::with_capacity(raw_buf_len);
            let mut upsampled_buffer: Vec<Complex<f32>> = Vec::with_capacity(4800);
            if sample_rate != 48000 {
                needs_upsampling = true;
                println!("Detected lower sample rate...calculating upsampling parameters...\nSample Rate: {}", sample_rate);
            }

            let mut calc = LUFSCalculator {
                filter: KFilter::new(),
                filtered_buf: VecDeque::with_capacity(19200), //400ms with 48kHz -> 0.4 * 48000 = 19200
                rx_chan,
                tx_chan,
            };

            while let Ok(val) = calc.rx_chan.recv() {
                if needs_upsampling {
                    raw_buffer.push(Complex{re: val, im: 0.0});
                    if raw_buffer.len() == raw_buf_len {
                        fft.process(&mut raw_buffer);
                        upsampled_buffer.extend_from_slice(&raw_buffer[..raw_buf_len/2]);
                        // pad the centre of the freq domain to up sample
                        upsampled_buffer.extend( std::iter::repeat(Complex{re: 0.0, im:0.0}).take(4800 - raw_buf_len));
                        upsampled_buffer.extend_from_slice(&raw_buffer[raw_buf_len/2..]);
                        ifft.process(&mut upsampled_buffer);
                        //Process
                        upsampled_buffer.iter().for_each(|v| calc.filtered_buf.push_back(calc.filter.next(v.re/(raw_buf_len as f32))));
                        raw_buffer.clear();
                        upsampled_buffer.clear();
                    }
                } else {
                    calc.filtered_buf.push_back(calc.filter.next(val));
                }
                if calc.filtered_buf.len() == 19200 {
                    // calculate and clear 25% of the buffer
                    match calc.tx_chan.send(-0.691 + 10.*(calc.filtered_buf.iter().map(|x| x*x).sum::<f32>()/19200.).log10()) {
                        Ok(_) => (),
                        Err(e) => break,
                    }
                    calc.filtered_buf.drain(0..4800);
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
    fn test_48kHzSample_1kHz() {
        let (tx_chan, rx_chan) = channel();
        let (tx2_chan, rx2_chan) = channel();

        LUFSCalculator::start(rx_chan, tx2_chan, 48000);
        for i in 0..19200 {
            tx_chan.send(((i as f32) * 2.0 * PI * 997.0 / 48000.).sin()).expect("should not happen");
        }
        match rx2_chan.recv() {
            Ok(v) => assert!(v > -3.02 && v < -3.0), // should be -3.01
            Err(e) => panic!("Should not happen"),
        }
    }

    #[test]
    fn test_16kHzSample_1kHz() {
        let (tx_chan, rx_chan) = channel();
        let (tx2_chan, rx2_chan) = channel();

        LUFSCalculator::start(rx_chan, tx2_chan, 16000);
        for i in 0..6400 {
            tx_chan.send(((i as f32) * 2.0 * PI * 997.0 / 16000.).sin()).expect("should not happen");
        }
        match rx2_chan.recv() {
            Ok(v) => assert!(v > -3.02 && v < -3.0), // should be -3.01
            Err(e) => panic!("Should not happen"),
        }
    }
}