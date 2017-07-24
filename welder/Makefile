ex: build
	adb push target/arm-linux-androideabi/debug/welder /data/
	adb shell /data/welder
build:
	export CC=/home/aep/android/build/android-18-toolchain/bin/arm-linux-androideabi-clang && \
	cargo rustc  --target arm-linux-androideabi  -- -C linker=$$CC
