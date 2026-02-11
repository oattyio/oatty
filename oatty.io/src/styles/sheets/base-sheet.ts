import rawCss from '../css/base.css?inline';
import { createConstructibleStyle } from './constructible-style';

export const baseStyle = createConstructibleStyle(rawCss);
