BIN := kelly
DIST := dist

.PHONY: build clean install uninstall

build:
	cargo build --release
	mkdir -p $(DIST)
	cp target/release/$(BIN) $(DIST)/$(BIN)
	@echo "Built $(BIN) to $(DIST)/"

install: build
	mkdir -p $(HOME)/.local/bin
	cp $(DIST)/$(BIN) $(HOME)/.local/bin/$(BIN)
	@echo "Installed $(BIN) to ~/.local/bin/"

uninstall:
	rm -f $(HOME)/.local/bin/$(BIN)
	@echo "Removed $(BIN) from ~/.local/bin/"

clean:
	cargo clean
	rm -rf $(DIST)