import rawCss from '../css/theme.css?inline';
import { createConstructibleStyle } from './constructible-style';

export const themeStyle = createConstructibleStyle(rawCss);
