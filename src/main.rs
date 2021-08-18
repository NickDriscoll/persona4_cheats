use std::env;
use std::mem::size_of;
use std::process::exit;
use std::thread::sleep;
use std::time::{Duration, Instant};
use winapi::um::tlhelp32;
use winapi::um::memoryapi::{ReadProcessMemory, WriteProcessMemory};
use winapi::um::processthreadsapi::OpenProcess;
use winapi::um::winnt;
use winapi::ctypes::c_void;

//Memory info
const TEAM_BASE_ADDR: u32 = 0x04DDD034;
const HEALTH_OFFSET: u32 = 0x08;
const SP_OFFSET: u32 = 0x0A;
const TEAM_STRIDE: u32 = 0x84;

//u16
const KNOWLEDGE_ADDR: u32 = 0x4DDD068;
const COURAGE_ADDR: u32 = 0x4DDD06A;
const DILIGENCE_ADDR: u32 = 0x4DDD06C;
const UNDERSTANDING_ADDR: u32 = 0x4DDD06E;
const EXPRESSION_ADDR: u32 = 0x4DDD070;

//Social link counters
const YOSUKE_SOCIAL_LINK: u32 = 0x04DDDCB4;
//const YOSUKE_SOCIAL_LINK: u32 = 0x04DDDCC4;
const CHIE_SOCIAL_LINK: u32 = 0x04DDDCD4;
const SPORTS_SOCIAL_LINK: u32 = 0x04DDDCE4;		// Daisuke/Kou
const ARTS_SOCIAL_LINK: u32 = 0x04DDDCF4;		// Yumi/annoying band girl
const NANAKO_SOCIAL_LINK: u32 = 0x04DDDD04;
const YOSUKE_SOCIAL_LINK: u32 = 0x04DDDD14;
const YUKIKO_SOCIAL_LINK: u32 = 0x04DDDD24;
const ADACHI_SOCIAL_LINK: u32 = 0x04DDDD34;
const DOJIMA_SOCIAL_LINK: u32 = 0x04DDDD44;
//const ADACHI_SOCIAL_LINK: u32 = 0x04DDDD54;
const SHU_SOCIAL_LINK: u32 = 0x04DDDD64;

const CHIE_XP_ADDR: u32 = 0x04DDD198;

const BATTLE_BASE_ADDR: u32 = 0x00AF1680;
const ENEMY_ARRAY_OFFSET: u32 = 0xCE8;
const ENEMY_HEALTH_OFFSET: u32 = 0x14;
const ENEMY_SP_OFFSET: u32 = 0x16;
const ENEMY_STRIDE: u32 = 0x34;

const ITEMS_BASE_ADDR: u32 = 0x04DDC6F2;
const SOMA_OFFSET: u32 = 0x16;
const SMART_BOMB_OFFSET: u32 = 0x2C;

const ITEM_STRINGS_BASE_ADDR: u32 = 0x00DAFD08;
const ITEM_STRINGS_STRIDE: u32 = 0x18;

//Set to 01000000 to be undetectable
//I'm assuming this is a bitmask but I've never seen it take a value other than 0 or 01000000 so eh
const DETECTABILITY_FLAG_ADDR: u32 = 0x04DDD6F3;
const INVISIBLE_TO_SHADOWS: u32 = 0b01000000;

const MOVEMENT_BASE_PTR: u32 = 0x21EB49A4;
const PLAYER_XPOS_OFFSET: u32 = 0x270;
const PLAYER_YPOS_OFFSET: u32 = 0x274;
const PLAYER_ZPOS_OFFSET: u32 = 0x278;

//The process name we search for
const EXE_NAME: &str = "P4G.exe";

fn clear_buffer(array: &mut [winnt::CHAR]) {
	for i in 0..array.len() {
		array[i] = 0;
	}
}

fn get_exe_name(proc_struct: &tlhelp32::PROCESSENTRY32) -> String {
	let mut st = String::new();
	for c in proc_struct.szExeFile.iter() {
		st.push(*c as u8 as char);
	}
	//Trim trailing null bytes
	st.trim_matches(char::from(0)).to_string()
}

fn read_f32(process_handle: winnt::HANDLE, address: u32) -> f32 {
	let mut bytes_touched = 0;
	let mut buffer = [0; 4];
	unsafe {
		ReadProcessMemory(
			process_handle,
			address as *mut c_void,
			&mut buffer as *mut _ as *mut c_void,
			4,
			&mut bytes_touched
		);
	}
	f32::from_le_bytes(buffer)
}

fn read_int(process_handle: winnt::HANDLE, address: u32, num_bytes: u32) -> u32 {
	let mut bytes_touched = 0;
	let mut buffer = 0;
	unsafe {
		ReadProcessMemory(
			process_handle,
			address as *mut c_void,
			&mut buffer as *mut _ as *mut c_void,
			num_bytes as usize,
			&mut bytes_touched
		);
	}
	buffer
}

fn read_string_bytes(process_handle: winnt::HANDLE, address: u32, num_bytes: u32) -> Vec<u8> {
	let mut bytes_touched = 0;
	let mut buffer = [0x0; ITEM_STRINGS_STRIDE as usize];
	unsafe {
		ReadProcessMemory(
			process_handle,
			address as *mut c_void,
			&mut buffer as *mut _ as *mut c_void,
			num_bytes as usize,
			&mut bytes_touched
		);
	}

	let mut res = Vec::with_capacity(24);
	let mut i = 0;
	while buffer[i] != 0x0 {
		res.push(buffer[i]);
		i += 1;
	}
	res
}

fn write_float(process_handle: winnt::HANDLE, address: u32, buffer: f32) {
	let mut bytes_touched = 0;
	unsafe {
		WriteProcessMemory(
			process_handle,
			address as *mut c_void,
			&buffer as *const _ as *const c_void,
			4,
			&mut bytes_touched
		);
	}
}

fn write_int(process_handle: winnt::HANDLE, address: u32, num_bytes: u32, buffer: u32) {
	let mut bytes_touched = 0;
	unsafe {
		WriteProcessMemory(
			process_handle,
			address as *mut c_void,
			&buffer as *const _ as *const c_void,
			num_bytes as usize,
			&mut bytes_touched
		);
	}
}

fn main() {
	//Flags
	let mut displayed_strings = false;

	let mut saved_xpos = 0.0;
	let mut saved_ypos = 0.0;
	let mut saved_zpos = 0.0;
	let mut saved_xp = 0;

	//First thing is to open the Persona 4 process
	println!("Searching for {}...", EXE_NAME);
	let process_handle = unsafe {
		let mut proc_struct = tlhelp32::PROCESSENTRY32 { 
			dwSize: size_of::<tlhelp32::PROCESSENTRY32>() as u32, //This is so fucking dumb, Microsoft
			cntUsage: 0,
			th32ProcessID: 0,
			th32DefaultHeapID: 0,
			th32ModuleID: 0,
			cntThreads: 0,
			th32ParentProcessID: 0,
			pcPriClassBase: 0,
			dwFlags: 0,
			szExeFile: [0; 260]
		};

		let mut found_process = false;
		while !found_process {
			//Assuming it won't be the first process, because it won't be
			let snap_handle = tlhelp32::CreateToolhelp32Snapshot(tlhelp32::TH32CS_SNAPPROCESS, 0);
			tlhelp32::Process32First(snap_handle, &mut proc_struct);
			clear_buffer(&mut proc_struct.szExeFile);
			while tlhelp32::Process32Next(snap_handle, &mut proc_struct) != 0 {
				if get_exe_name(&proc_struct).eq_ignore_ascii_case(EXE_NAME) {
					println!("{}'s process ID is {}", EXE_NAME, proc_struct.th32ProcessID);
					found_process = true;
					break;
				}

				clear_buffer(&mut proc_struct.szExeFile);
			}
		}

		//Open the process with read/write access
		OpenProcess(winnt::PROCESS_ALL_ACCESS, 0, proc_struct.th32ProcessID)
	};

	//Some variables for keeping track of time
	let mut last_frame_instant = Instant::now();
	let mut elapsed_time = 0.0;

	//Write the values we want to memory over and over forever
	loop {
		let delta_time = {
			const MAX_DELTA_TIME: f32 = 1.0 / 30.0;
			let frame_instant = Instant::now();
			let dur = frame_instant.duration_since(last_frame_instant);
			last_frame_instant = frame_instant;
			let f_dur = dur.as_secs_f32();

			//Don't allow game objects to have an update delta of more than a thirtieth of a second
			if f_dur > MAX_DELTA_TIME { MAX_DELTA_TIME }
			else { f_dur }
		};
		elapsed_time += delta_time;

		//Finding the pointer to the first enemy
		let first_enemy_base_addr = {
			let ptr = read_int(process_handle, BATTLE_BASE_ADDR, 4);
			read_int(process_handle, ptr + ENEMY_ARRAY_OFFSET, 4)
		};

		//Make all enemies have 1hp and 0sp
		if first_enemy_base_addr != 0x0 {
			for i in 0..6 {
				write_int(process_handle, first_enemy_base_addr + ENEMY_HEALTH_OFFSET + i * ENEMY_STRIDE, 2, 1);
				write_int(process_handle, first_enemy_base_addr + ENEMY_SP_OFFSET + i * ENEMY_STRIDE, 2, 0);
			}
		}

		//Write max social stats
		//Addrs start at knowledge and are tightly-packed u16s
		for i in 0..5 {
			write_int(process_handle, KNOWLEDGE_ADDR + i * 0x02, 2, 500);
		}

		//Write item amounts
		write_int(process_handle, ITEMS_BASE_ADDR + SOMA_OFFSET, 1, 69);
		write_int(process_handle, ITEMS_BASE_ADDR + SMART_BOMB_OFFSET, 1, 69);

		//Write undetectability
		write_int(process_handle, DETECTABILITY_FLAG_ADDR, 1, INVISIBLE_TO_SHADOWS);

		//Turbo speed
		{
			let scale = 2.5;
			let base = read_int(process_handle, MOVEMENT_BASE_PTR, 4);

			let offsets = [PLAYER_XPOS_OFFSET, PLAYER_ZPOS_OFFSET];
			let saved_pos = [&mut saved_xpos, &mut saved_zpos];
			for i in 0..offsets.len() {
				let pos = read_f32(process_handle, base + offsets[i]);
				if *saved_pos[i] != 0.0 {
					let diff = f32::abs(pos - *saved_pos[i]);
					if diff > 0.00001 && diff < 100.0 {
						let new_pos = pos + scale * (pos - *saved_pos[i]);
						write_float(process_handle, base + offsets[i], new_pos);
						*saved_pos[i] = new_pos;
					} else {
						*saved_pos[i] = pos;
					}
				} else {
					*saved_pos[i] = pos;
				}
			}
		}

		//XP boost
		{
			let multiplier = 5;
			let xp = read_int(process_handle, CHIE_XP_ADDR, 4);
			if saved_xp != 0 {
				let diff = xp - saved_xp;
				if diff > 0 {
					let new_xp = xp + (multiplier - 1) * diff;
					write_int(process_handle, CHIE_XP_ADDR, 4, new_xp);
					saved_xp = new_xp;
				} else {
					saved_xp = xp;
				}
			} else {
				saved_xp = xp;
			}
		}

		//SP fuckery
		{
			let sp_mid_value = 65205;
			let max_offset = 600.0;
			for i in 0..8 {
				let frequency = 5.0 * (i + 1) as f32;
				let offset = (max_offset * f32::sin(elapsed_time * frequency) + max_offset) / 2.0;
				write_int(
					process_handle,
					TEAM_BASE_ADDR + SP_OFFSET + i * TEAM_STRIDE,
					2,
					sp_mid_value + offset as u32
				);
			}
		}

		//Display all item strings once
		/*
		if (!displayed_strings) {
			print!("[");
			for i in 0..0xA00 {
				let item_name_bytes = read_string_bytes(process_handle, ITEM_STRINGS_BASE_ADDR + i * ITEM_STRINGS_STRIDE, ITEM_STRINGS_STRIDE);
				match String::from_utf8(item_name_bytes) {
					Ok(st) => {
						if st.chars().nth(0).unwrap() != '0' && st != "Blank" {
							print!("0x{:X}, ", i);
							//write_int(process_handle, ITEMS_BASE_ADDR + i, 1, 69);
						}
					}
					Err(e) => {
						println!("{}", e);
					}
				}
			}
			println!("]");

			displayed_strings = true;
		}
		*/

		//sleep to avoid throttling the CPU
		sleep(Duration::from_millis(5));
	}
}
