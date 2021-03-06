#!/usr/bin/env bash

bold=$(tput bold)
normal=$(tput sgr0)

black=$(tput setaf 0)
red=$(tput setaf 1)
green=$(tput setaf 2)
blue=$(tput setaf 4)

set -e

for dir in `find . -maxdepth 1 -type d`; do
	if [ $dir == "." ]; then continue; fi
	if [ $dir == "./bootloader" ]; then continue; fi
	if [ $dir == "./old_revisions" ]; then continue; fi
	echo "${bold}${blue}Compiling${black} $dir${normal}"
	pushd $dir > /dev/null
	make
	popd > /dev/null
done

echo ""
echo "${bold}All built successfully!${normal}"
