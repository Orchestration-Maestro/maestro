;; MIT controlled engine. Pixels expose its tick count and last key event.
(component
  (type $bytes (list u8))
  (type $outcome (result (error string)))
  (core module $module
    (memory (export "memory") 32)
    (global $heap (mut i32) (i32.const 1100000))
    (global $ticks (mut i32) (i32.const 0))
    (global $key (mut i32) (i32.const 0))
    (global $held (mut i32) (i32.const 0))
    (data (i32.const 64) "empty WAD")
    (data (i32.const 80) "unexpected game clock")
    (func (export "realloc") (param i32 i32) (param $align i32) (param $size i32) (result i32)
      (local $pointer i32)
      global.get $heap
      local.get $align i32.const 1 i32.sub i32.add
      local.get $align i32.const 1 i32.sub i32.const -1 i32.xor i32.and
      local.tee $pointer local.get $size i32.add global.set $heap
      local.get $pointer)
    (func (export "init") (param i32) (param $length i32) (param i32 i32) (result i32)
      i32.const 0 i32.const 0 i32.store
      local.get $length i32.eqz
      if
        i32.const 0 i32.const 1 i32.store
        i32.const 4 i32.const 64 i32.store
        i32.const 8 i32.const 9 i32.store
      end
      i32.const 0 global.set $ticks
      i32.const 0 global.set $key
      i32.const 0 global.set $held
      i32.const 0)
    (func (export "tick") (param $milliseconds i32) (result i32)
      i32.const 0 i32.const 0 i32.store
      global.get $ticks i32.const 1 i32.add i32.const 1000 i32.mul i32.const 35 i32.div_u
      local.get $milliseconds i32.ne
      if
        i32.const 0 i32.const 1 i32.store
        i32.const 4 i32.const 80 i32.store
        i32.const 8 i32.const 21 i32.store
      else
        global.get $ticks i32.const 1 i32.add global.set $ticks
      end
      i32.const 0)
    (func (export "key") (param $pressed i32) (param $code i32)
      local.get $pressed global.set $held
      local.get $code global.set $key)
    (func (export "dimensions") (result i32)
      i32.const 40 i32.const 640 i32.store
      i32.const 44 i32.const 400 i32.store
      i32.const 40)
    (func (export "frame") (result i32)
      (local $offset i32)
      loop $pixel
        local.get $offset i32.const 4096 i32.add
        global.get $ticks local.get $offset i32.const 4 i32.div_u i32.const 640 i32.rem_u i32.add i32.store8
        local.get $offset i32.const 4097 i32.add
        global.get $key local.get $offset i32.const 2560 i32.div_u i32.const 20 i32.div_u i32.add i32.store8
        local.get $offset i32.const 4098 i32.add global.get $held i32.const 255 i32.mul i32.store8
        local.get $offset i32.const 4099 i32.add i32.const 255 i32.store8
        local.get $offset i32.const 4 i32.add local.tee $offset
        i32.const 1024000 i32.lt_u br_if $pixel
      end
      i32.const 32 i32.const 4096 i32.store
      i32.const 36 i32.const 1024000 i32.store
      i32.const 32))
  (core instance $core (instantiate $module))
  (func $init (param "wad" $bytes) (param "root" string) (result $outcome)
    (canon lift (core func $core "init") (memory (core memory $core "memory")) (realloc (core func $core "realloc"))))
  (func $tick (param "milliseconds" u32) (result $outcome)
    (canon lift (core func $core "tick") (memory (core memory $core "memory")) (realloc (core func $core "realloc"))))
  (func $dimensions (result (tuple u32 u32))
    (canon lift (core func $core "dimensions") (memory (core memory $core "memory"))))
  (func $frame (result $bytes)
    (canon lift (core func $core "frame") (memory (core memory $core "memory")) (realloc (core func $core "realloc"))))
  (func $key (param "pressed" bool) (param "key" u8)
    (canon lift (core func $core "key")))
  (instance $engine
    (export "init" (func $init))
    (export "tick" (func $tick))
    (export "dimensions" (func $dimensions))
    (export "frame" (func $frame))
    (export "key" (func $key)))
  (export "maestro:doom/engine@0.1.0" (instance $engine)))
