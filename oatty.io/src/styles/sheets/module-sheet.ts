import rawCss from '../css/module.css?inline';
import { createConstructibleStyle } from './constructible-style';

export const moduleStyle = createConstructibleStyle(rawCss);
