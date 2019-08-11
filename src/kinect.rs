
#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]

extern crate libc;

use libc::{size_t, c_char};
use std::borrow::{Borrow, BorrowMut};
use std::ffi::{CString};
use std::mem::MaybeUninit;
use std::os::raw::c_uint;
use std::ptr::{self, null_mut, null};
use std::slice;
use byteorder::{LittleEndian, ByteOrder};
use std::collections::VecDeque;
include!(concat!(env!("OUT_DIR"), "/bindings.rs"));

pub struct K4ADevice {
	device: k4a_device_t,
	capture_config: k4a_device_configuration_t,
	serial_number: Option<String>, // Starts as None but becomes Some lazily.
	tracker: Option<k4abt_tracker_t>,
	active_captures: VecDeque<K4ACapture>
}

pub struct K4ACapture {
	capture: k4a_capture_t,
	image: Option<k4a_image_t>,
	frame: Option<k4abt_frame_t>,
	skeleton: Option<k4abt_skeleton_t>
}

impl K4ADevice {
	// Build a new device, start the tracker, and run.
	pub fn new() -> Self {
		let mut device:Self = K4ADevice::try_open(K4A_DEVICE_DEFAULT).expect("Failed to open device.");
		device.start_capture();
		device.start_tracker();
		return device;
	}
	
	pub fn next_frame(&mut self) {
		// Queues a capture and does all the processing on it.
		let cap = self.get_capture(-1).expect("Failed to synchronously queue frame.");
		//self.queue_capture_for_processing(&cap, -1);
		self.active_captures.push_back(cap);
		println!("Grabbing new frame.  Buffer size: {}", self.active_captures.len());
	}
	
	pub fn drop_oldest_capture(&mut self) {
		if let Some(cap) = self.active_captures.pop_front() {
			std::mem::drop(cap);
			println!("Dropping oldest cap.");
		}
	}
	
	pub fn get_depth_image(&mut self) -> (usize, usize, Vec<u16>) {
		let capture_obj: &mut K4ACapture = self.active_captures.front_mut().expect("No captures queued.");
		let image:k4a_image_t = match capture_obj.image {
			Some(img) => img,
			None => {
				unsafe {
					let img: k4a_image_t = k4a_capture_get_depth_image(capture_obj.capture);
					capture_obj.image = Some(img);
					img
				}
			}
		};
		
		unsafe {
			let height = k4a_image_get_height_pixels(image) as usize;
			let width = k4a_image_get_width_pixels(image) as usize;
			let stride = k4a_image_get_stride_bytes(image) as usize;
			let size = k4a_image_get_size(image) as usize;
			
			let mut result = Vec::<u16>::with_capacity(size/2);
			result.set_len(size/2);
			let buff:*const u8 = k4a_image_get_buffer(image);
			let s = slice::from_raw_parts(buff, size);
			//ptr::copy(buff, result.as_mut_ptr(), size);
			LittleEndian::read_u16_into(s, result.as_mut_slice());
			// Rust won't touch the ptr, so we don't need to mem::forget.
			// It won't get dropped when it goes out of scope, but it's a pointer to the image data.
			return (height, width, result);
		}
	}
	
	pub fn get_skeleton(&self) -> () {
	
	}
	
	//
	// Internals
	//
	
	// Get a Device.  The default one.  Any one.  May panic.
	fn try_open(device_id:u32) -> Result<Self, String> {
		eprintln!("Trying to count devices.");
		if device_get_installed_count() == 0 {
			eprintln!("No devices available.");
			return Result::Err("No devices installed.".to_string());
		}
		
		eprintln!("Allocating space for devices.");
		let mut k4a_device = MaybeUninit::<k4a_device_t>::uninit();
		eprintln!("Allocated!");
		
		unsafe {
			eprintln!("Unsafe open device.");
			if k4a_device_open(device_id as c_uint, k4a_device.as_mut_ptr()) != k4a_result_t::K4A_RESULT_SUCCEEDED {
				eprintln!("device_open failed.");
				return Result::Err("Unable to open device.".to_string());
			}
			
			eprintln!("Success: Opened device.");
			return Result::Ok(
				K4ADevice {
					device: unsafe { k4a_device.assume_init() },
					capture_config: k4a_device_configuration_t {
						camera_fps: k4a_fps_t::K4A_FRAMES_PER_SECOND_30,
						color_format: k4a_image_format_t::K4A_IMAGE_FORMAT_COLOR_MJPG,
						color_resolution: k4a_color_resolution_t::K4A_COLOR_RESOLUTION_OFF,
						depth_delay_off_color_usec: 0i32,
						depth_mode: k4a_depth_mode_t::K4A_DEPTH_MODE_WFOV_2X2BINNED, // For 30FPS, can't use full 1024x1024.
						disable_streaming_indicator: false,
						subordinate_delay_off_master_usec:0u32, // Ignored
						synchronized_images_only:false, // Only capturing depth.
						wired_sync_mode:k4a_wired_sync_mode_t::K4A_WIRED_SYNC_MODE_STANDALONE
					},
					serial_number: None,
					tracker: None,
					active_captures: VecDeque::new()
				}
			);
		}
	}
	
	pub fn get_serial_number(&mut self) -> String {
		match &mut self.serial_number {
			Some(n) => n.clone(),
			None => {
				unsafe {
					//size_t serial_size = 0;
					//k4a_device_get_serialnum(device, NULL, &serial_size)
					let mut serial_size:size_t = 0;
					k4a_device_get_serialnum(self.device, null_mut(), &mut serial_size);
					println!("Serial size: {}", &serial_size);
					let mut string_memory = Vec::<u8>::with_capacity(serial_size as usize);
					k4a_device_get_serialnum(self.device, string_memory.as_mut_ptr() as *mut i8, &mut serial_size);
					self.serial_number = Some(CString::from_vec_unchecked(string_memory).into_string().unwrap_or("ERROR UNWRAPPING SERIAL NUMBER".to_string()));
					println!("Got serial number: {}", self.serial_number.clone().unwrap());
				}
				return self.serial_number.clone().expect("ERROR: EMPTY SERIAL NUMBER").clone();
			}
		}
	}
	
	fn set_config(&mut self, config:k4a_device_configuration_t) {
		self.capture_config = config;
	}
	
	// BGRA32 uses extra CPU.  It is not a native format for the device.
	fn start_capture(&mut self) {
		unsafe {
			let config_ptr: *mut k4a_device_configuration_t = &mut self.capture_config;
			k4a_device_start_cameras(self.device, config_ptr);
		}
	}
	
	fn start_tracker(&mut self) {
		unsafe {
			//k4a_calibration_t sensor_calibration;
			//k4a_device_get_calibration(device, deviceConfig.depth_mode, K4A_COLOR_RESOLUTION_OFF, &sensor_calibration);
			let mut uninit_sensor_calibration = MaybeUninit::<k4a_calibration_t>::uninit();
			let device_get_result = k4a_device_get_calibration(self.device, self.capture_config.depth_mode, k4a_color_resolution_t::K4A_COLOR_RESOLUTION_OFF, uninit_sensor_calibration.as_mut_ptr());
			let sensor_calibration = uninit_sensor_calibration.assume_init();
			assert_eq!(device_get_result, k4a_result_t::K4A_RESULT_SUCCEEDED);
			
			//k4abt_tracker_t tracker = NULL;
			//k4abt_tracker_create(&sensor_calibration, &tracker);
			let mut uninit_tracker = MaybeUninit::<k4abt_tracker_t>::uninit();
			let tracker_create_result = k4abt_tracker_create(&sensor_calibration, uninit_tracker.as_mut_ptr());
			assert_eq!(tracker_create_result, k4a_result_t::K4A_RESULT_SUCCEEDED);
			self.tracker = Some(uninit_tracker.assume_init());
		}
	}
	
	// Use -1 for infinite wait for synchronous capture.
	fn get_capture(&mut self, wait_time:i32) -> Result<K4ACapture, i32> {
		unsafe {
			let mut uninit_capture = MaybeUninit::<k4a_capture_t>::uninit();
			let res:i32 = k4a_device_get_capture(self.device, uninit_capture.as_mut_ptr(), wait_time);
			match res {
				k4a_wait_result_t::K4A_WAIT_RESULT_SUCCEEDED => Ok(K4ACapture {
					capture: uninit_capture.assume_init(),
					image: None,
					frame: None,
					skeleton: None
				}),
				e => Err(e) // _RESULT_FAILED, _RESULT_TIMEOUT
			}
		}
	}
	
	// wait_time == -1 for infinite wait.
	fn queue_capture_for_processing(&mut self, cap: &K4ACapture, wait_time:i32) -> i32 {
		unsafe{
			//K4ABT_EXPORT k4a_wait_result_t k4abt_tracker_enqueue_capture(k4abt_tracker_t tracker_handle, k4a_capture_t sensor_capture_handle, int32_t timeout_in_ms);
			k4abt_tracker_enqueue_capture(self.tracker.expect("Tracker has not been initialized.  Did you call start tracker?"), cap.capture, wait_time)
		}
	}
	
	// wait_time == -1 for infinite wait.  Maybe fills the K4A capture with k4abt_frame data.
	fn fetch_tracking_result(&mut self, cap: &mut K4ACapture, wait_time:i32) -> i32 {
		unsafe {
			let mut uninit_frame = MaybeUninit::<k4abt_frame_t>::uninit();
			let res = k4abt_tracker_pop_result(self.tracker.expect("Tracker uninitialized."), uninit_frame.as_mut_ptr(), wait_time);
			if res == k4a_wait_result_t::K4A_WAIT_RESULT_SUCCEEDED {
				cap.frame = Some(uninit_frame.assume_init());
			}
			return res;
		}
	}
	
	fn get_skeleton_result(&mut self, cap: &mut K4ACapture, wait_time:i32) -> i32 {
		return 0;
	}
}

impl Drop for K4ADevice {
	fn drop(&mut self) {
		unsafe {
			match self.tracker {
				Some(t) => {
					k4abt_tracker_shutdown(t);
					k4abt_tracker_destroy(t);
				},
				None => ()
			};
			
			k4a_device_stop_cameras(self.device);
			k4a_device_close(self.device);
		}
	}
}

impl Drop for K4ACapture {
	fn drop(&mut self) {
		unsafe {
			if let Some(im) = self.image {
				k4a_image_release(im);
			}
			if let Some(frame) = self.frame {
				k4abt_frame_release(frame);
			}
			k4a_capture_release(self.capture);
		}
	}
}

pub fn device_get_installed_count() -> u32 {
	unsafe {
		return k4a_device_get_installed_count() as u32;
	}
	return 0;
}

#[cfg(test)]
mod tests {
	use super::*;
	
	#[test]
	fn do_it() {
		let device_count = device_get_installed_count();
		assert_eq!(device_count, 1);
	}
	
	#[test]
	fn get_serial_number() {
		println!("Starting get_serial_number test");
		let mut k4a = K4ADevice::try_open(Default::default()).expect("ERROR: Couldn't open Kinect device.");
		assert_ne!(k4a.get_serial_number(), "ERROR UNWRAPPING SERIAL NUMBER");
		assert!(k4a.get_serial_number().is_ascii());
		println!("Finished get serial number.");
	}
}