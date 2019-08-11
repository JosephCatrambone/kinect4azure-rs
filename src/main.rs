#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]

extern crate libc;
extern crate minifb;

use minifb::{Key, WindowOptions, Window};

const WIDTH: usize = 512;
const HEIGHT: usize = 512;

mod kinect;

use kinect::*;
use std::cmp::{min, max};

fn main() {
	let mut counter:u64 = 0;
	let mut k4a = kinect::K4ADevice::new();
	
	let mut buffer: Vec<u32> = vec![0; WIDTH * HEIGHT];
	
	let mut window = Window::new("Test - ESC to exit", WIDTH, HEIGHT,
		WindowOptions::default()).unwrap_or_else(|e| {
			panic!("{}", e);
		}
	);
	
	k4a.next_frame();
	let (height, width, mut depth) = k4a.get_depth_image();
	while window.is_open() && !window.is_key_down(Key::Escape) {
		let mut trigger_update = false;
		
		counter = (counter+1)%10;
		if counter == 0 {
			trigger_update = true;
		}
		
		// Find the max to normalize.
		let (min, max) = (&depth).into_iter().fold((0xFFFF, 0), |acc, v:&u16|{
			(min(*v, acc.0), max(*v, acc.1))
		});
		let scaling_factor = 1.0f64 / ((max - min) as f64);
		
		for (index, pixel) in buffer.iter_mut().enumerate() {
			let pixel_x = index % WIDTH;
			let pixel_y = index / HEIGHT;
			let depth_index = pixel_y*width + pixel_x; // Transpose
			*pixel = (255f64*(((depth[depth_index] as f64) - min as f64)*scaling_factor)) as u32; //(*pixel + index as u32)%65536;
		}
		
		// We unwrap here as we want this code to exit if it fails. Real applications may want to handle this in a different way
		window.update_with_buffer(&buffer).unwrap();
		
		window.get_keys().map(|keys| {
			for t in keys {
				match t {
					Key::Enter => {
						trigger_update = true;
					}
					Key::W => println!("W"),
					_ => (),
				}
			}
		});
		
		// Get a frame.
		if trigger_update {
			k4a.drop_oldest_capture();
			k4a.next_frame();
			depth = k4a.get_depth_image().2;
		}
	}
}

