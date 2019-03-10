use cpal;

use keys_state::KeysState;
use std::sync::mpsc::{channel, Receiver, Sender};
use types::{KeyAction, SignalProcessorFunction};

pub struct CpalEngineController {
    key_action_sender: Sender<KeyAction>,
    signal_processor_change_sender: Sender<SignalProcessorFunction>,
}

impl CpalEngineController {
    pub fn start() -> Self {
        let (key_action_sender, key_action_receiver) = channel::<KeyAction>();
        let (signal_processor_change_sender, signal_processor_change_receiver) =
            channel::<SignalProcessorFunction>();

        start_audio_thread(key_action_receiver, signal_processor_change_receiver);

        Self {
            key_action_sender,
            signal_processor_change_sender,
        }
    }

    pub fn set_processor_function(
        &self,
        new_func: Box<FnMut(f64, f64, Option<i32>) -> f64 + Send>,
    ) {
        self.signal_processor_change_sender.send(new_func).unwrap();
    }

    pub fn key_action(&mut self, action: KeyAction) {
        self.key_action_sender.send(action).unwrap();
    }
}

fn start_audio_thread(
    key_action_receiver: Receiver<KeyAction>,
    signal_processor_change_receiver: Receiver<SignalProcessorFunction>,
) {
    std::thread::spawn(move || {
        let device = cpal::default_output_device().expect("Failed to get default output device");
        let format = device
            .default_output_format()
            .expect("Failed to get default output format");
        let event_loop = cpal::EventLoop::new();
        let stream_id = event_loop.build_output_stream(&device, &format).unwrap();
        event_loop.play_stream(stream_id.clone());

        let mut key_action = None;
        let mut keys_state = KeysState::new();
        let mut audio_processor_function: Box<FnMut(f64, f64, Option<i32>) -> f64 + Send> =
            Box::new(|duration, _, _| (duration * 440.0 * 2.0 * std::f64::consts::PI).sin());

        let sample_rate = format.sample_rate.0 as f32;
        let sample_time = (1.0 / sample_rate) as f64;
        let mut duration = 0.0;

        event_loop.run(move |_, data| {
            for _new_processor in signal_processor_change_receiver.try_iter() {
                audio_processor_function = _new_processor;
            }

            for _key in key_action_receiver.try_iter() {
                key_action = keys_state.key_down(_key);
            }

            match data {
                cpal::StreamData::Output {
                    buffer: cpal::UnknownTypeOutputBuffer::U16(mut buffer),
                } => {
                    for sample in buffer.chunks_mut(format.channels as usize) {
                        let value = ((audio_processor_function(duration, sample_time, key_action)
                            * 0.5
                            + 0.5)
                            * std::u16::MAX as f64) as u16;
                        for out in sample.iter_mut() {
                            *out = value;
                        }
                        duration += sample_time;
                    }
                }
                cpal::StreamData::Output {
                    buffer: cpal::UnknownTypeOutputBuffer::I16(mut buffer),
                } => {
                    for sample in buffer.chunks_mut(format.channels as usize) {
                        let value = ((audio_processor_function(duration, sample_time, key_action)
                            * 0.5
                            + 0.5)
                            * std::i16::MAX as f64) as i16;
                        for out in sample.iter_mut() {
                            *out = value;
                        }
                        duration += sample_time;
                    }
                }
                cpal::StreamData::Output {
                    buffer: cpal::UnknownTypeOutputBuffer::F32(mut buffer),
                } => {
                    for sample in buffer.chunks_mut(format.channels as usize) {
                        let value =
                            audio_processor_function(duration, sample_time, key_action) as f32;
                        for out in sample.iter_mut() {
                            *out = value;
                        }
                        duration += sample_time;
                    }
                }
                _ => (),
            }
        });
    });
}