#!/usr/bin/bash

# aws
export AWS_PROFILE=<YOUR_AWS_PROFILE_HERE>

#global
outfolder="./out"
infolder="./in"

shopt -s globstar nullglob

rx='^[a-z0-9._-]+$'

for filename in $infolder/*.{MP3,mp3,M4A,m4a,MP4,mp4,OGG,ogg,WEBM,webm,WAV,wav,FLAC,flac,AMR,amr}; do
    printf "Verifying %s\n" "$filename"
    stem=$( basename "${filename}" )
    if [[ ! "${stem}" =~ $rx ]]; then
        printf "NOT OK, Fixing: %s\n" "$stem"
        ./translit-mv "$filename"
    else
        printf "OK: %s\n" "$filename"
    fi
done

printf "\n"

for filename in $infolder/*.{mp3,m4a,mp4,ogg,webm,wav,flac,amr}; do
    stem=$( basename "${filename%.*}" )
    distill-cli -i "${filename}" -o text -s "$outfolder/${stem}"
done
