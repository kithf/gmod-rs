use std::{mem::MaybeUninit, borrow::Cow, ffi::c_void};
use crate::{userdata::TaggedUserData, lua::*};

unsafe fn handle_pcall_ignore(lua: State) {
	crate::lua_stack_guard!(lua => {
		lua.get_global(crate::lua_string!("ErrorNoHaltWithStack"));
		if lua.is_nil(-1) {
			eprintln!("[ERROR] {:?}", lua.get_string(-2));
			lua.pop();
		} else {
			#[cfg(debug_assertions)] {
				lua.push_string(&format!("[pcall_ignore] {}", lua.get_string(-2).expect("Expected a string here")));
			}
			#[cfg(not(debug_assertions))] {
				lua.push_value(-2);
			}

			lua.call(1, 0);
		}
	});
	lua.pop();
}

#[repr(transparent)]
#[derive(Clone, Copy, Debug)]
pub struct LuaState(pub *mut std::ffi::c_void);

impl LuaState {
	pub unsafe fn new() -> Result<Self, LuaError> {
		let lua = (LUA_SHARED.lual_newstate)();
		(LUA_SHARED.lual_openlibs)(lua);
		if lua.is_null() {
			Err(LuaError::MemoryAllocationError)
		} else {
			Ok(lua)
		}
	}

	/// Returns whether this is the clientside Lua state or not.
	pub unsafe fn is_client(&self) -> bool {
		self.get_global(crate::lua_string!("CLIENT"));
		let client = self.get_boolean(-1);
		self.pop();
		client
	}

	/// Returns whether this is the serverside Lua state or not.
	pub unsafe fn is_server(&self) -> bool {
		self.get_global(crate::lua_string!("SERVER"));
		let server = self.get_boolean(-1);
		self.pop();
		server
	}

	/// Returns whether this is the menu Lua state or not.
	pub unsafe fn is_menu(&self) -> bool {
		self.get_global(crate::lua_string!("MENU_DLL"));
		let menu = self.get_boolean(-1);
		self.pop();
		menu
	}

	/// Returns the Lua string as a slice of bytes.
	///
	/// **WARNING:** This will CHANGE the type of the value at the given index to a string.
	///
	/// Returns None if the value at the given index is not convertible to a string.
	pub unsafe fn get_binary_string(&self, index: i32) -> Option<&[u8]> {
		let mut len: usize = 0;
		let ptr = (LUA_SHARED.lua_tolstring)(*self, index, &mut len);

		if ptr.is_null() {
			return None;
		}

		Some(std::slice::from_raw_parts(ptr as *const u8, len))
	}

	/// Returns the Lua string as a Rust UTF-8 String.
	///
	/// **WARNING:** This will CHANGE the type of the value at the given index to a string.
	///
	/// Returns None if the value at the given index is not convertible to a string.
	///
	/// This is a lossy operation, and will replace any invalid UTF-8 sequences with the Unicode replacement character. See the documentation for `String::from_utf8_lossy` for more information.
	///
	/// If you need raw data, use `get_binary_string`.
	pub unsafe fn get_string(&self, index: i32) -> Option<std::borrow::Cow<'_, str>> {
		let mut len: usize = 0;
		let ptr = (LUA_SHARED.lua_tolstring)(*self, index, &mut len);

		if ptr.is_null() {
			return None;
		}

		let bytes = std::slice::from_raw_parts(ptr as *const u8, len);

		Some(String::from_utf8_lossy(bytes))
	}

	/// Returns the name of the type of the value at the given index.
	pub unsafe fn get_type(&self, index: i32) -> &str {
		let lua_type = (LUA_SHARED.lua_type)(*self, index);
		let lua_type_str_ptr = (LUA_SHARED.lua_typename)(*self, lua_type);
		let lua_type_str = std::ffi::CStr::from_ptr(lua_type_str_ptr);
		unsafe { std::str::from_utf8_unchecked(lua_type_str.to_bytes()) }
	}

	#[inline(always)]
	pub unsafe fn get_top(&self) -> i32 {
		(LUA_SHARED.lua_gettop)(*self)
	}

	#[inline(always)]
	/// Pops the stack, inserts the value into the registry table, and returns the registry index of the value.
	///
	/// Use `from_reference` with the reference index to push the value back onto the stack.
	///
	/// Use `dereference` to free the reference from the registry table.
	pub unsafe fn reference(&self) -> LuaReference {
		(LUA_SHARED.lual_ref)(*self, LUA_REGISTRYINDEX)
	}

	#[inline(always)]
	pub unsafe fn dereference(&self, r#ref: LuaReference) {
		(LUA_SHARED.lual_unref)(*self, LUA_REGISTRYINDEX, r#ref)
	}

	#[inline(always)]
	pub unsafe fn from_reference(&self, r#ref: LuaReference) {
		self.raw_geti(LUA_REGISTRYINDEX, r#ref)
	}

	#[inline(always)]
	/// You may be looking for `is_none_or_nil`
	pub unsafe fn is_nil(&self, index: i32) -> bool {
		(LUA_SHARED.lua_type)(*self, index) == LUA_TNIL
	}

	#[inline(always)]
	pub unsafe fn is_none(&self, index: i32) -> bool {
		(LUA_SHARED.lua_type)(*self, index) == LUA_TNONE
	}

	#[inline(always)]
	pub unsafe fn is_none_or_nil(&self, index: i32) -> bool {
		self.is_nil(index) || self.is_none(index)
	}

	#[inline(always)]
	pub unsafe fn is_function(&self, index: i32) -> bool {
		(LUA_SHARED.lua_type)(*self, index) == LUA_TFUNCTION
	}

	#[inline(always)]
	pub unsafe fn is_table(&self, index: i32) -> bool {
		(LUA_SHARED.lua_type)(*self, index) == LUA_TTABLE
	}

	#[inline(always)]
	pub unsafe fn is_boolean(&self, index: i32) -> bool {
		(LUA_SHARED.lua_type)(*self, index) == LUA_TBOOLEAN
	}

	#[inline(always)]
	pub unsafe fn remove(&self, index: i32) {
		(LUA_SHARED.lua_remove)(*self, index)
	}

	#[inline(always)]
	pub unsafe fn push_value(&self, index: i32) {
		(LUA_SHARED.lua_pushvalue)(*self, index)
	}

	#[inline(always)]
	pub unsafe fn push_lightuserdata(&self, data: *mut c_void) {
		(LUA_SHARED.lua_pushlightuserdata)(*self, data)
	}

	#[inline(always)]
	pub unsafe fn get_field(&self, index: i32, k: LuaString) {
		(LUA_SHARED.lua_getfield)(*self, index, k)
	}

	#[inline(always)]
	pub unsafe fn push_boolean(&self, boolean: bool) {
		(LUA_SHARED.lua_pushboolean)(*self, if boolean { 1 } else { 0 })
	}

	#[inline(always)]
	pub unsafe fn push_integer(&self, int: LuaInt) {
		(LUA_SHARED.lua_pushinteger)(*self, int)
	}

	#[inline(always)]
	pub unsafe fn push_number(&self, num: LuaNumber) {
		(LUA_SHARED.lua_pushnumber)(*self, num)
	}

	#[inline(always)]
	pub unsafe fn push_nil(&self) {
		(LUA_SHARED.lua_pushnil)(*self)
	}

	#[inline(always)]
	pub unsafe fn push_thread(&self) -> i32 {
		(LUA_SHARED.lua_pushthread)(*self)
	}

	#[inline(always)]
	pub unsafe fn to_thread(&self, index: i32) -> State {
		(LUA_SHARED.lua_tothread)(*self, index)
	}

	#[inline(always)]
	pub unsafe fn pcall(&self, nargs: i32, nresults: i32, errfunc: i32) -> i32 {
		(LUA_SHARED.lua_pcall)(*self, nargs, nresults, errfunc)
	}

	/// Same as pcall, but ignores any runtime error and calls `ErrorNoHaltWithStack` instead with the error message.
	///
	/// Returns whether the execution was successful.
	pub unsafe fn pcall_ignore(&self, nargs: i32, nresults: i32) -> bool {
		match self.pcall(nargs, nresults, 0) {
			LUA_OK => true,
			LUA_ERRRUN => {
				handle_pcall_ignore(*self);
				false
			}
			err => {
				#[cfg(debug_assertions)]
				eprintln!("[gmod-rs] pcall_ignore unknown error: {}", err);
				false
			}
		}
	}

	pub unsafe fn load_string(&self, src: LuaString) -> Result<(), LuaError> {
		let lua_error_code = (LUA_SHARED.lual_loadstring)(*self, src);
		if lua_error_code == 0 {
			Ok(())
		} else {
			Err(LuaError::from_lua_state(*self, lua_error_code))
		}
	}

	pub unsafe fn load_buffer(&self, buff: &[u8], name: LuaString) -> Result<(), LuaError> {
		let lua_error_code = (LUA_SHARED.lual_loadbuffer)(*self, buff.as_ptr() as LuaString, buff.len(), name);
		if lua_error_code == 0 {
			Ok(())
		} else {
			Err(LuaError::from_lua_state(*self, lua_error_code))
		}
	}

	pub unsafe fn load_file(&self, path: LuaString) -> Result<(), LuaError> {
		let lua_error_code = (LUA_SHARED.lual_loadfile)(*self, path);
		if lua_error_code == 0 {
			Ok(())
		} else {
			Err(LuaError::from_lua_state(*self, lua_error_code))
		}
	}

	#[inline(always)]
	pub unsafe fn pop(&self) {
		self.pop_n(1);
	}

	#[inline(always)]
	pub unsafe fn pop_n(&self, count: i32) {
		self.set_top(-count - 1);
	}

	#[inline(always)]
	pub unsafe fn set_top(&self, index: i32) {
		(LUA_SHARED.lua_settop)(*self, index)
	}

	#[inline(always)]
	pub unsafe fn lua_type(&self, index: i32) -> i32 {
		(LUA_SHARED.lua_type)(*self, index)
	}

	pub unsafe fn lua_type_name(&self, lua_type_id: i32) -> Cow<'_, str> {
		let hackfix = self.get_top(); // https://github.com/Facepunch/garrysmod-issues/issues/5134

		let type_str_ptr = (LUA_SHARED.lua_typename)(*self, lua_type_id);

		self.pop_n((self.get_top() - hackfix).max(0));

		let type_str = std::ffi::CStr::from_ptr(type_str_ptr);
		type_str.to_string_lossy()
	}

	#[inline(always)]
	pub unsafe fn replace(&self, index: i32) {
		(LUA_SHARED.lua_replace)(*self, index)
	}

	#[inline(always)]
	pub unsafe fn push_globals(&self) {
		(LUA_SHARED.lua_pushvalue)(*self, LUA_GLOBALSINDEX)
	}

	#[inline(always)]
	pub unsafe fn push_registry(&self) {
		(LUA_SHARED.lua_pushvalue)(*self, LUA_REGISTRYINDEX)
	}

	#[inline(always)]
	pub unsafe fn push_string(&self, data: &str) {
		(LUA_SHARED.lua_pushlstring)(*self, data.as_ptr() as LuaString, data.len())
	}

	#[inline(always)]
	pub unsafe fn push_binary_string(&self, data: &[u8]) {
		(LUA_SHARED.lua_pushlstring)(*self, data.as_ptr() as LuaString, data.len())
	}

	#[inline(always)]
	pub unsafe fn push_function(&self, func: LuaFunction) {
		(LUA_SHARED.lua_pushcclosure)(*self, func, 0)
	}

	#[inline(always)]
	/// Creates a closure, which can be used as a function with stored data (upvalues)
	///
	/// ## Example
	///
	/// ```ignore
	/// #[lua_function]
	/// unsafe fn foo(lua: gmod::lua::State) {
	///     lua.get_closure_arg(1);
	///     let hello = lua.get_string(-1);
	///     println!("{}", hello);
	/// }
	///
	/// lua.push_string("Hello, world!");
	/// lua.push_closure(foo, 1);
	/// ```
	pub unsafe fn push_closure(&self, func: LuaFunction, n: i32) {
		debug_assert!(n <= 255, "Can't push more than 255 arguments into a closure");
		(LUA_SHARED.lua_pushcclosure)(*self, func, n)
	}

	#[inline(always)]
	/// Pushes the `n`th closure argument onto the stack
	///
	/// ## Example
	///
	/// ```ignore
	/// #[lua_function]
	/// unsafe fn foo(lua: gmod::lua::State) {
	///     lua.push_closure_arg(1);
	///     let hello = lua.get_string(-1);
	///     println!("{}", hello);
	/// }
	///
	/// lua.push_string("Hello, world!");
	/// lua.push_closure(foo, 1);
	/// ```
	pub unsafe fn push_closure_arg(&self, n: i32) {
		self.push_value(self.upvalue_index(n));
	}

	#[inline(always)]
	/// Equivalent to C `lua_upvalueindex` macro
	pub const fn upvalue_index(&self, idx: i32) -> i32 {
		LUA_GLOBALSINDEX - idx
	}

	#[inline(always)]
	pub unsafe fn set_table(&self, index: i32) {
		(LUA_SHARED.lua_settable)(*self, index)
	}

	#[inline(always)]
	pub unsafe fn set_field(&self, index: i32, k: LuaString) {
		(LUA_SHARED.lua_setfield)(*self, index, k)
	}

	#[inline(always)]
	pub unsafe fn get_global(&self, name: LuaString) {
		(LUA_SHARED.lua_getfield)(*self, LUA_GLOBALSINDEX, name)
	}

	#[inline(always)]
	pub unsafe fn set_global(&self, name: LuaString) {
		(LUA_SHARED.lua_setfield)(*self, LUA_GLOBALSINDEX, name)
	}

	#[inline(always)]
	/// WARNING: Any Lua errors caused by calling the function will longjmp and prevent any further execution of your code.
	///
	/// To workaround this, use `pcall_ignore`, which will call `ErrorNoHaltWithStack` instead and allow your code to continue executing.
	pub unsafe fn call(&self, nargs: i32, nresults: i32) {
		(LUA_SHARED.lua_call)(*self, nargs, nresults)
	}

	#[inline(always)]
	pub unsafe fn insert(&self, index: i32) {
		(LUA_SHARED.lua_insert)(*self, index)
	}

	/// Creates a new table and pushes it to the stack.
	/// seq_n is a hint as to how many sequential elements the table may have.
	/// hash_n is a hint as to how many non-sequential/hashed elements the table may have.
	/// Lua may use these hints to preallocate memory.
	#[inline(always)]
	pub unsafe fn create_table(&self, seq_n: i32, hash_n: i32) {
		(LUA_SHARED.lua_createtable)(*self, seq_n, hash_n)
	}

	/// Creates a new table and pushes it to the stack without memory preallocation hints.
	/// Equivalent to `create_table(0, 0)`
	#[inline(always)]
	pub unsafe fn new_table(&self) {
		(LUA_SHARED.lua_createtable)(*self, 0, 0)
	}

	#[inline(always)]
	pub unsafe fn get_table(&self, index: i32) {
		(LUA_SHARED.lua_gettable)(*self, index)
	}

	pub unsafe fn check_binary_string(&self, arg: i32) -> &[u8] {
		let mut len: usize = 0;
		let ptr = (LUA_SHARED.lual_checklstring)(*self, arg, &mut len);
		std::slice::from_raw_parts(ptr as *const u8, len)
	}

	pub unsafe fn check_string(&self, arg: i32) -> Cow<'_, str> {
		let mut len: usize = 0;
		let ptr = (LUA_SHARED.lual_checklstring)(*self, arg, &mut len);
		String::from_utf8_lossy(std::slice::from_raw_parts(ptr as *const u8, len))
	}

	#[inline(always)]
	pub unsafe fn check_userdata(&self, arg: i32, name: LuaString) -> *mut TaggedUserData {
		(LUA_SHARED.lual_checkudata)(*self, arg, name) as *mut _
	}

	pub unsafe fn test_userdata(&self, index: i32, name: LuaString) -> bool {
		if !(LUA_SHARED.lua_touserdata)(*self, index).is_null() && self.get_metatable(index) != 0 {
			self.get_field(LUA_REGISTRYINDEX, name);
			let result = self.raw_equal(-1, -2);
			self.pop_n(2);
			if result {
				return true;
			}
		}
		false
	}

	#[inline(always)]
	pub unsafe fn raw_equal(&self, a: i32, b: i32) -> bool {
		(LUA_SHARED.lua_rawequal)(*self, a, b) == 1
	}

	#[inline(always)]
	pub unsafe fn get_metatable(&self, index: i32) -> i32 {
		(LUA_SHARED.lua_getmetatable)(*self, index)
	}

	#[inline(always)]
	pub unsafe fn check_table(&self, arg: i32) {
		(LUA_SHARED.lual_checktype)(*self, arg, LUA_TTABLE)
	}

	#[inline(always)]
	pub unsafe fn check_function(&self, arg: i32) {
		(LUA_SHARED.lual_checktype)(*self, arg, LUA_TFUNCTION)
	}

	#[inline(always)]
	pub unsafe fn check_integer(&self, arg: i32) -> LuaInt {
		(LUA_SHARED.lual_checkinteger)(*self, arg)
	}

	#[inline(always)]
	pub unsafe fn check_number(&self, arg: i32) -> f64 {
		(LUA_SHARED.lual_checknumber)(*self, arg)
	}

	#[inline(always)]
	pub unsafe fn check_boolean(&self, arg: i32) -> bool {
		(LUA_SHARED.lual_checktype)(*self, arg, LUA_TBOOLEAN);
		(LUA_SHARED.lua_toboolean)(*self, arg) == 1
	}

	#[inline(always)]
	pub unsafe fn to_integer(&self, index: i32) -> LuaInt {
		(LUA_SHARED.lua_tointeger)(*self, index)
	}

	#[inline(always)]
	pub unsafe fn to_number(&self, index: i32) -> f64 {
		(LUA_SHARED.lua_tonumber)(*self, index)
	}

	#[inline(always)]
	pub unsafe fn get_boolean(&self, index: i32) -> bool {
		(LUA_SHARED.lua_toboolean)(*self, index) == 1
	}

	#[inline(always)]
	pub unsafe fn set_metatable(&self, index: i32) -> i32 {
		(LUA_SHARED.lua_setmetatable)(*self, index)
	}

	#[inline(always)]
	#[allow(clippy::len_without_is_empty)]
	pub unsafe fn len(&self, index: i32) -> i32 {
		(LUA_SHARED.lua_objlen)(*self, index)
	}

	#[inline(always)]
	pub unsafe fn raw_get(&self, index: i32) {
		(LUA_SHARED.lua_rawget)(*self, index)
	}

	#[inline(always)]
	pub unsafe fn raw_geti(&self, t: i32, index: i32) {
		(LUA_SHARED.lua_rawgeti)(*self, t, index)
	}

	#[inline(always)]
	pub unsafe fn raw_set(&self, index: i32) {
		(LUA_SHARED.lua_rawset)(*self, index)
	}

	#[inline(always)]
	pub unsafe fn raw_seti(&self, t: i32, index: i32) {
		(LUA_SHARED.lua_rawseti)(*self, t, index)
	}

	#[inline(always)]
	pub unsafe fn next(&self, index: i32) -> i32 {
		(LUA_SHARED.lua_next)(*self, index)
	}

	#[inline(always)]
	pub unsafe fn to_pointer(&self, index: i32) -> *const c_void {
		(LUA_SHARED.lua_topointer)(*self, index)
	}

	#[inline(always)]
	pub unsafe fn to_userdata(&self, index: i32) -> *mut c_void {
		(LUA_SHARED.lua_touserdata)(*self, index)
	}

	#[inline(always)]
	pub unsafe fn coroutine_new(&self) -> State {
		(LUA_SHARED.lua_newthread)(*self)
	}

	#[inline(always)]
	#[must_use]
	pub unsafe fn coroutine_yield(&self, nresults: i32) -> i32 {
		(LUA_SHARED.lua_yield)(*self, nresults)
	}

	#[inline(always)]
	#[must_use]
	pub unsafe fn coroutine_resume(&self, narg: i32) -> i32 {
		(LUA_SHARED.lua_resume)(*self, narg)
	}

	#[inline(always)]
	/// Exchange values between different threads of the same global state.
	///
	/// This function pops `n` values from the stack `self`, and pushes them onto the stack `target_thread`.
	pub unsafe fn coroutine_exchange(&self, target_thread: State, n: i32) {
		(LUA_SHARED.lua_xmove)(*self, target_thread, n)
	}

	#[inline(always)]
	pub unsafe fn equal(&self, index1: i32, index2: i32) -> bool {
		(LUA_SHARED.lua_equal)(*self, index1, index2) == 1
	}

	#[inline(always)]
	/// See `call`
	pub unsafe fn coroutine_resume_call(&self, narg: i32) {
		match (LUA_SHARED.lua_resume)(*self, narg) {
			LUA_OK => {},
			LUA_ERRRUN => self.error(self.get_string(-2).unwrap_or(Cow::Borrowed("Unknown error")).as_ref()),
			LUA_ERRMEM => self.error("Out of memory"),
			_ => self.error("Unknown internal Lua error")
		}
	}

	#[inline(always)]
	/// See `pcall_ignore`
	pub unsafe fn coroutine_resume_pcall_ignore(&self, narg: i32) -> Result<i32, ()> {
		match (LUA_SHARED.lua_resume)(*self, narg) {
			status @ (LUA_OK | LUA_YIELD) => Ok(status),
			LUA_ERRRUN => {
				handle_pcall_ignore(*self);
				Err(())
			},
			err => {
				#[cfg(debug_assertions)]
				eprintln!("[gmod-rs] coroutine_resume_pcall_ignore unknown error: {}", err);
				Err(())
			}
		}
	}

	#[inline(always)]
	pub unsafe fn coroutine_status(&self) -> i32 {
		(LUA_SHARED.lua_status)(*self)
	}

	/// Creates a new table in the registry with the given `name` as the key if it doesn't already exist, and pushes it onto the stack.
	///
	/// Returns if the metatable was already present in the registry.
	#[inline(always)]
	pub unsafe fn new_metatable(&self, name: LuaString) -> bool {
		(LUA_SHARED.lual_newmetatable)(*self, name) == 0
	}

	pub unsafe fn new_userdata<T: Sized>(&self, data: T, metatable: Option<i32>) -> *mut T {
		let has_metatable = if std::mem::needs_drop::<T>() {
			if let Some(metatable) = metatable {
				self.push_value(metatable);
			} else {
				self.new_table();
			}
			self.push_function(crate::userdata::__gc::<T>);
			self.set_field(-2, crate::lua_string!("__gc"));
			true
		} else if let Some(metatable) = metatable {
			self.push_value(metatable);
			true
		} else {
			false
		};

		let ptr = (LUA_SHARED.lua_newuserdata)(*self, std::mem::size_of::<T>()) as *mut T;

		debug_assert_eq!(ptr as usize % std::mem::align_of::<T>(), 0, "Lua userdata is unaligned!");

		if has_metatable {
			self.push_value(-2);
			self.set_metatable(-2);
			self.remove(self.get_top() - 1);
			self.remove(self.get_top() - 1);
		}

		ptr.write(data);
		ptr
	}

	#[cold]
	pub unsafe fn error<S: AsRef<str>>(&self, msg: S) -> ! {
		self.push_string(msg.as_ref());
		(LUA_SHARED.lua_error)(*self);
		unreachable!()
	}

	pub unsafe fn debug_getinfo_from_ar(&self, ar: &mut LuaDebug, what: LuaString) -> Result<(), ()> {
		if (LUA_SHARED.lua_getinfo)(*self, what, ar as *mut LuaDebug) != 0 {
			Ok(())
		} else {
			Err(())
		}
	}

	/// `what` should start with `>` and pop a function off the stack
	pub unsafe fn debug_getinfo_from_stack(&self, what: LuaString) -> Option<LuaDebug> {
		let mut ar = MaybeUninit::uninit();
		if (LUA_SHARED.lua_getinfo)(*self, what, ar.as_mut_ptr()) != 0 {
			Some(ar.assume_init())
		} else {
			None
		}
	}

	pub unsafe fn get_stack_at(&self, level: i32) -> Option<LuaDebug> {
		let mut ar = MaybeUninit::uninit();
		if (LUA_SHARED.lua_getstack)(*self, level, ar.as_mut_ptr()) != 0 {
			Some(ar.assume_init())
		} else {
			None
		}
	}

	pub unsafe fn debug_getinfo_at(&self, level: i32, what: LuaString) -> Option<LuaDebug> {
		let mut ar = MaybeUninit::uninit();
		if (LUA_SHARED.lua_getstack)(*self, level, ar.as_mut_ptr()) != 0 && (LUA_SHARED.lua_getinfo)(*self, what, ar.as_mut_ptr()) != 0 {
			return Some(ar.assume_init());
		}
		None
	}

	pub unsafe fn dump_stack(&self) {
		let top = self.get_top();
		println!("\n=== STACK DUMP ===");
		println!("Stack size: {}", top);
		for i in 1..=top {
			let lua_type = self.lua_type(i);
			let lua_type_name = self.lua_type_name(lua_type);
			match lua_type_name.as_ref() {
				"string" => println!("{}. {}: {:?}", i, lua_type_name, {
					self.push_value(i);
					let str = self.get_string(-1);
					self.pop();
					str
				}),
				"boolean" => println!("{}. {}: {:?}", i, lua_type_name, {
					self.push_value(i);
					let bool = self.get_boolean(-1);
					self.pop();
					bool
				}),
				"number" => println!("{}. {}: {:?}", i, lua_type_name, {
					self.push_value(i);
					let n = self.to_number(-1);
					self.pop();
					n
				}),
				_ => println!("{}. {}", i, lua_type_name),
			}
		}
		println!();
	}

	pub unsafe fn dump_val(&self, index: i32) -> String {
		let lua_type_name = self.lua_type_name(self.lua_type(index));
		match lua_type_name.as_ref() {
			"string" => {
				self.push_value(index);
				let str = self.get_string(-1);
				self.pop();
				format!("{:?}", str.unwrap().into_owned())
			},
			"boolean" => {
				self.push_value(index);
				let boolean = self.get_boolean(-1);
				self.pop();
				format!("{}", boolean)
			},
			"number" => {
				self.push_value(index);
				let n = self.to_number(-1);
				self.pop();
				format!("{}", n)
			},
			_ => lua_type_name.into_owned(),
		}
	}
}
impl std::ops::Deref for LuaState {
	type Target = *mut std::ffi::c_void;

	#[inline(always)]
	fn deref(&self) -> &Self::Target {
		&self.0
	}
}
