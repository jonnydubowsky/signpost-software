# default board and architecture
TOCK_BOARD ?= controller
TOCK_ARCH ?= cortex-m4


# rules for making the kernel
.PHONY: all
all: $(TOCK_BOARD)

$(TOCK_BOARD): boards/$(TOCK_BOARD)/
	$(MAKE) -C $<

clean: boards/$(TOCK_BOARD)/
	$(MAKE) clean -C $<

doc: boards/$(TOCK_BOARD)/
	$(MAKE) doc -C $<

debug: boards/$(TOCK_BOARD)/
	$(MAKE) debug -C $<

program: boards/$(TOCK_BOARD)/
	$(MAKE) program -C $<

flash: boards/$(TOCK_BOARD)/
	$(MAKE) flash -C $<


# rule for making userland example applications
apps/%: ../apps/%
	$(MAKE) -C $< TOCK_ARCH=$(TOCK_ARCH)
	$(MAKE) flash -C $< TOCK_ARCH=$(TOCK_ARCH)

