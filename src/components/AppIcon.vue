<script setup lang="ts">
import { computed, type CSSProperties } from "vue";
import purewallLogo from "../assets/purewall-icon.svg";
import addIcon from "@fluentui/svg-icons/icons/add_24_regular.svg";
import arrowDownloadIcon from "@fluentui/svg-icons/icons/arrow_download_24_regular.svg";
import arrowNextIcon from "@fluentui/svg-icons/icons/arrow_next_24_regular.svg";
import arrowSortIcon from "@fluentui/svg-icons/icons/arrow_sort_24_regular.svg";
import arrowUploadIcon from "@fluentui/svg-icons/icons/arrow_upload_24_regular.svg";
import calendarIcon from "@fluentui/svg-icons/icons/calendar_24_regular.svg";
import checkmarkIcon from "@fluentui/svg-icons/icons/checkmark_24_regular.svg";
import chevronDownIcon from "@fluentui/svg-icons/icons/chevron_down_24_regular.svg";
import chevronRightIcon from "@fluentui/svg-icons/icons/chevron_right_24_regular.svg";
import copyIcon from "@fluentui/svg-icons/icons/copy_24_regular.svg";
import dataBarVerticalIcon from "@fluentui/svg-icons/icons/data_bar_vertical_24_regular.svg";
import databaseIcon from "@fluentui/svg-icons/icons/database_24_regular.svg";
import darkThemeIcon from "@fluentui/svg-icons/icons/dark_theme_24_regular.svg";
import deleteIcon from "@fluentui/svg-icons/icons/delete_24_regular.svg";
import desktopIcon from "@fluentui/svg-icons/icons/desktop_24_regular.svg";
import dismissIcon from "@fluentui/svg-icons/icons/dismiss_24_regular.svg";
import documentIcon from "@fluentui/svg-icons/icons/document_24_regular.svg";
import eyeOffIcon from "@fluentui/svg-icons/icons/eye_off_24_regular.svg";
import filterIcon from "@fluentui/svg-icons/icons/filter_24_regular.svg";
import folderIcon from "@fluentui/svg-icons/icons/folder_24_regular.svg";
import fullScreenMaximizeIcon from "@fluentui/svg-icons/icons/full_screen_maximize_24_regular.svg";
import fullScreenMinimizeIcon from "@fluentui/svg-icons/icons/full_screen_minimize_24_regular.svg";
import gridDotsIcon from "@fluentui/svg-icons/icons/grid_dots_24_regular.svg";
import gridIcon from "@fluentui/svg-icons/icons/grid_24_regular.svg";
import heartFilledIcon from "@fluentui/svg-icons/icons/heart_24_filled.svg";
import heartIcon from "@fluentui/svg-icons/icons/heart_24_regular.svg";
import imageIcon from "@fluentui/svg-icons/icons/image_24_regular.svg";
import keyboardIcon from "@fluentui/svg-icons/icons/keyboard_24_regular.svg";
import libraryIcon from "@fluentui/svg-icons/icons/library_24_regular.svg";
import listIcon from "@fluentui/svg-icons/icons/list_24_regular.svg";
import moreHorizontalIcon from "@fluentui/svg-icons/icons/more_horizontal_24_regular.svg";
import openIcon from "@fluentui/svg-icons/icons/open_24_regular.svg";
import optionsIcon from "@fluentui/svg-icons/icons/options_24_regular.svg";
import pauseIcon from "@fluentui/svg-icons/icons/pause_24_regular.svg";
import personIcon from "@fluentui/svg-icons/icons/person_24_regular.svg";
import panelRightIcon from "@fluentui/svg-icons/icons/panel_right_24_regular.svg";
import playIcon from "@fluentui/svg-icons/icons/play_24_regular.svg";
import resizeImageIcon from "@fluentui/svg-icons/icons/resize_image_24_regular.svg";
import searchIcon from "@fluentui/svg-icons/icons/search_24_regular.svg";
import selectAllOnIcon from "@fluentui/svg-icons/icons/select_all_on_24_regular.svg";
import settingsIcon from "@fluentui/svg-icons/icons/settings_24_regular.svg";
import shieldIcon from "@fluentui/svg-icons/icons/shield_24_regular.svg";
import tagIcon from "@fluentui/svg-icons/icons/tag_24_regular.svg";
import tagSearchIcon from "@fluentui/svg-icons/icons/tag_search_24_regular.svg";
import thumbDislikeIcon from "@fluentui/svg-icons/icons/thumb_dislike_24_regular.svg";
import commentIcon from "@fluentui/svg-icons/icons/comment_24_regular.svg";
import wallpaperIcon from "@fluentui/svg-icons/icons/wallpaper_24_regular.svg";
import weatherSunnyIcon from "@fluentui/svg-icons/icons/weather_sunny_24_regular.svg";
import windowIcon from "@fluentui/svg-icons/icons/window_24_regular.svg";

const props = defineProps<{
  name: string;
}>();

const icons: Record<string, string> = {
  logo: purewallLogo,
  PureWallLogo: purewallLogo,
  library: libraryIcon,
  archive: libraryIcon,
  heart: heartIcon,
  like: heartIcon,
  hidden: eyeOffIcon,
  "eye-off": eyeOffIcon,
  tag: tagIcon,
  "tag-search": tagSearchIcon,
  "panel-right": panelRightIcon,
  widget: windowIcon,
  display: desktopIcon,
  desktop: desktopIcon,
  settings: settingsIcon,
  "theme-dark": darkThemeIcon,
  "theme-light": weatherSunnyIcon,
  advanced: optionsIcon,
  shortcuts: keyboardIcon,
  next: arrowNextIcon,
  "skip-forward": arrowNextIcon,
  dislike: thumbDislikeIcon,
  minus: thumbDislikeIcon,
  pause: pauseIcon,
  play: playIcon,
  filter: filterIcon,
  grid: gridIcon,
  compact: gridDotsIcon,
  list: listIcon,
  more: moreHorizontalIcon,
  folder: folderIcon,
  "quiet-canvas": fullScreenMaximizeIcon,
  "exit-quiet": fullScreenMinimizeIcon,
  wallpaper: wallpaperIcon,
  sort: arrowSortIcon,
  select: selectAllOnIcon,
  calendar: calendarIcon,
  person: personIcon,
  author: personIcon,
  shield: shieldIcon,
  copyright: shieldIcon,
  comment: commentIcon,
  resolution: resizeImageIcon,
  "file-size": documentIcon,
  FileSize: documentIcon,
  copy: copyIcon,
  storage: databaseIcon,
  download: arrowDownloadIcon,
  upload: arrowUploadIcon,
  chart: dataBarVerticalIcon,
  "external-link": openIcon,
  ExternalLink: openIcon,
  search: searchIcon,
  image: imageIcon,
  check: checkmarkIcon,
  plus: addIcon,
  trash: deleteIcon,
  "chevron-right": chevronRightIcon,
  "chevron-down": chevronDownIcon,
  x: dismissIcon,
};

const iconStyle = computed(() => {
  const icon = icons[props.name] ?? imageIcon;
  const isLogo = props.name === "logo" || props.name === "PureWallLogo";

  return {
    "--icon-mask": `url("${icon}")`,
    "--icon-mask-filled": `url("${props.name === "heart" || props.name === "like" ? heartFilledIcon : icon}")`,
    ...(isLogo
      ? {
          backgroundColor: "transparent",
          backgroundImage: `url("${icon}")`,
          backgroundPosition: "center",
          backgroundRepeat: "no-repeat",
          backgroundSize: "contain",
          WebkitMask: "none",
          mask: "none",
        }
      : {}),
  } as CSSProperties;
});
</script>

<template>
  <span
    aria-hidden="true"
    :class="['app-icon', `app-icon--${name}`]"
    :style="iconStyle"
  />
</template>
