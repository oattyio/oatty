(function(){const e=document.createElement("link").relList;if(e&&e.supports&&e.supports("modulepreload"))return;for(const r of document.querySelectorAll('link[rel="modulepreload"]'))a(r);new MutationObserver(r=>{for(const o of r)if(o.type==="childList")for(const i of o.addedNodes)i.tagName==="LINK"&&i.rel==="modulepreload"&&a(i)}).observe(document,{childList:!0,subtree:!0});function t(r){const o={};return r.integrity&&(o.integrity=r.integrity),r.referrerPolicy&&(o.referrerPolicy=r.referrerPolicy),r.crossOrigin==="use-credentials"?o.credentials="include":r.crossOrigin==="anonymous"?o.credentials="omit":o.credentials="same-origin",o}function a(r){if(r.ep)return;r.ep=!0;const o=t(r);fetch(r.href,o)}})();/**
 * @license
 * Copyright 2019 Google LLC
 * SPDX-License-Identifier: BSD-3-Clause
 */const E=globalThis,j=E.ShadowRoot&&(E.ShadyCSS===void 0||E.ShadyCSS.nativeShadow)&&"adoptedStyleSheets"in Document.prototype&&"replace"in CSSStyleSheet.prototype,ie=Symbol(),G=new WeakMap;let ve=class{constructor(e,t,a){if(this._$cssResult$=!0,a!==ie)throw Error("CSSResult is not constructable. Use `unsafeCSS` or `css` instead.");this.cssText=e,this.t=t}get styleSheet(){let e=this.o;const t=this.t;if(j&&e===void 0){const a=t!==void 0&&t.length===1;a&&(e=G.get(t)),e===void 0&&((this.o=e=new CSSStyleSheet).replaceSync(this.cssText),a&&G.set(t,e))}return e}toString(){return this.cssText}};const ye=s=>new ve(typeof s=="string"?s:s+"",void 0,ie),be=(s,e)=>{if(j)s.adoptedStyleSheets=e.map(t=>t instanceof CSSStyleSheet?t:t.styleSheet);else for(const t of e){const a=document.createElement("style"),r=E.litNonce;r!==void 0&&a.setAttribute("nonce",r),a.textContent=t.cssText,s.appendChild(a)}},K=j?s=>s:s=>s instanceof CSSStyleSheet?(e=>{let t="";for(const a of e.cssRules)t+=a.cssText;return ye(t)})(s):s;/**
 * @license
 * Copyright 2017 Google LLC
 * SPDX-License-Identifier: BSD-3-Clause
 */const{is:we,defineProperty:xe,getOwnPropertyDescriptor:ke,getOwnPropertyNames:_e,getOwnPropertySymbols:Se,getPrototypeOf:Ce}=Object,v=globalThis,Q=v.trustedTypes,$e=Q?Q.emptyScript:"",q=v.reactiveElementPolyfillSupport,z=(s,e)=>s,B={toAttribute(s,e){switch(e){case Boolean:s=s?$e:null;break;case Object:case Array:s=s==null?s:JSON.stringify(s)}return s},fromAttribute(s,e){let t=s;switch(e){case Boolean:t=s!==null;break;case Number:t=s===null?null:Number(s);break;case Object:case Array:try{t=JSON.parse(s)}catch{t=null}}return t}},ne=(s,e)=>!we(s,e),J={attribute:!0,type:String,converter:B,reflect:!1,useDefault:!1,hasChanged:ne};Symbol.metadata??(Symbol.metadata=Symbol("metadata")),v.litPropertyMetadata??(v.litPropertyMetadata=new WeakMap);let _=class extends HTMLElement{static addInitializer(e){this._$Ei(),(this.l??(this.l=[])).push(e)}static get observedAttributes(){return this.finalize(),this._$Eh&&[...this._$Eh.keys()]}static createProperty(e,t=J){if(t.state&&(t.attribute=!1),this._$Ei(),this.prototype.hasOwnProperty(e)&&((t=Object.create(t)).wrapped=!0),this.elementProperties.set(e,t),!t.noAccessor){const a=Symbol(),r=this.getPropertyDescriptor(e,a,t);r!==void 0&&xe(this.prototype,e,r)}}static getPropertyDescriptor(e,t,a){const{get:r,set:o}=ke(this.prototype,e)??{get(){return this[t]},set(i){this[t]=i}};return{get:r,set(i){const c=r==null?void 0:r.call(this);o==null||o.call(this,i),this.requestUpdate(e,c,a)},configurable:!0,enumerable:!0}}static getPropertyOptions(e){return this.elementProperties.get(e)??J}static _$Ei(){if(this.hasOwnProperty(z("elementProperties")))return;const e=Ce(this);e.finalize(),e.l!==void 0&&(this.l=[...e.l]),this.elementProperties=new Map(e.elementProperties)}static finalize(){if(this.hasOwnProperty(z("finalized")))return;if(this.finalized=!0,this._$Ei(),this.hasOwnProperty(z("properties"))){const t=this.properties,a=[..._e(t),...Se(t)];for(const r of a)this.createProperty(r,t[r])}const e=this[Symbol.metadata];if(e!==null){const t=litPropertyMetadata.get(e);if(t!==void 0)for(const[a,r]of t)this.elementProperties.set(a,r)}this._$Eh=new Map;for(const[t,a]of this.elementProperties){const r=this._$Eu(t,a);r!==void 0&&this._$Eh.set(r,t)}this.elementStyles=this.finalizeStyles(this.styles)}static finalizeStyles(e){const t=[];if(Array.isArray(e)){const a=new Set(e.flat(1/0).reverse());for(const r of a)t.unshift(K(r))}else e!==void 0&&t.push(K(e));return t}static _$Eu(e,t){const a=t.attribute;return a===!1?void 0:typeof a=="string"?a:typeof e=="string"?e.toLowerCase():void 0}constructor(){super(),this._$Ep=void 0,this.isUpdatePending=!1,this.hasUpdated=!1,this._$Em=null,this._$Ev()}_$Ev(){var e;this._$ES=new Promise(t=>this.enableUpdating=t),this._$AL=new Map,this._$E_(),this.requestUpdate(),(e=this.constructor.l)==null||e.forEach(t=>t(this))}addController(e){var t;(this._$EO??(this._$EO=new Set)).add(e),this.renderRoot!==void 0&&this.isConnected&&((t=e.hostConnected)==null||t.call(e))}removeController(e){var t;(t=this._$EO)==null||t.delete(e)}_$E_(){const e=new Map,t=this.constructor.elementProperties;for(const a of t.keys())this.hasOwnProperty(a)&&(e.set(a,this[a]),delete this[a]);e.size>0&&(this._$Ep=e)}createRenderRoot(){const e=this.shadowRoot??this.attachShadow(this.constructor.shadowRootOptions);return be(e,this.constructor.elementStyles),e}connectedCallback(){var e;this.renderRoot??(this.renderRoot=this.createRenderRoot()),this.enableUpdating(!0),(e=this._$EO)==null||e.forEach(t=>{var a;return(a=t.hostConnected)==null?void 0:a.call(t)})}enableUpdating(e){}disconnectedCallback(){var e;(e=this._$EO)==null||e.forEach(t=>{var a;return(a=t.hostDisconnected)==null?void 0:a.call(t)})}attributeChangedCallback(e,t,a){this._$AK(e,a)}_$ET(e,t){var o;const a=this.constructor.elementProperties.get(e),r=this.constructor._$Eu(e,a);if(r!==void 0&&a.reflect===!0){const i=(((o=a.converter)==null?void 0:o.toAttribute)!==void 0?a.converter:B).toAttribute(t,a.type);this._$Em=e,i==null?this.removeAttribute(r):this.setAttribute(r,i),this._$Em=null}}_$AK(e,t){var o,i;const a=this.constructor,r=a._$Eh.get(e);if(r!==void 0&&this._$Em!==r){const c=a.getPropertyOptions(r),n=typeof c.converter=="function"?{fromAttribute:c.converter}:((o=c.converter)==null?void 0:o.fromAttribute)!==void 0?c.converter:B;this._$Em=r;const p=n.fromAttribute(t,c.type);this[r]=p??((i=this._$Ej)==null?void 0:i.get(r))??p,this._$Em=null}}requestUpdate(e,t,a,r=!1,o){var i;if(e!==void 0){const c=this.constructor;if(r===!1&&(o=this[e]),a??(a=c.getPropertyOptions(e)),!((a.hasChanged??ne)(o,t)||a.useDefault&&a.reflect&&o===((i=this._$Ej)==null?void 0:i.get(e))&&!this.hasAttribute(c._$Eu(e,a))))return;this.C(e,t,a)}this.isUpdatePending===!1&&(this._$ES=this._$EP())}C(e,t,{useDefault:a,reflect:r,wrapped:o},i){a&&!(this._$Ej??(this._$Ej=new Map)).has(e)&&(this._$Ej.set(e,i??t??this[e]),o!==!0||i!==void 0)||(this._$AL.has(e)||(this.hasUpdated||a||(t=void 0),this._$AL.set(e,t)),r===!0&&this._$Em!==e&&(this._$Eq??(this._$Eq=new Set)).add(e))}async _$EP(){this.isUpdatePending=!0;try{await this._$ES}catch(t){Promise.reject(t)}const e=this.scheduleUpdate();return e!=null&&await e,!this.isUpdatePending}scheduleUpdate(){return this.performUpdate()}performUpdate(){var a;if(!this.isUpdatePending)return;if(!this.hasUpdated){if(this.renderRoot??(this.renderRoot=this.createRenderRoot()),this._$Ep){for(const[o,i]of this._$Ep)this[o]=i;this._$Ep=void 0}const r=this.constructor.elementProperties;if(r.size>0)for(const[o,i]of r){const{wrapped:c}=i,n=this[o];c!==!0||this._$AL.has(o)||n===void 0||this.C(o,void 0,i,n)}}let e=!1;const t=this._$AL;try{e=this.shouldUpdate(t),e?(this.willUpdate(t),(a=this._$EO)==null||a.forEach(r=>{var o;return(o=r.hostUpdate)==null?void 0:o.call(r)}),this.update(t)):this._$EM()}catch(r){throw e=!1,this._$EM(),r}e&&this._$AE(t)}willUpdate(e){}_$AE(e){var t;(t=this._$EO)==null||t.forEach(a=>{var r;return(r=a.hostUpdated)==null?void 0:r.call(a)}),this.hasUpdated||(this.hasUpdated=!0,this.firstUpdated(e)),this.updated(e)}_$EM(){this._$AL=new Map,this.isUpdatePending=!1}get updateComplete(){return this.getUpdateComplete()}getUpdateComplete(){return this._$ES}shouldUpdate(e){return!0}update(e){this._$Eq&&(this._$Eq=this._$Eq.forEach(t=>this._$ET(t,this[t]))),this._$EM()}updated(e){}firstUpdated(e){}};_.elementStyles=[],_.shadowRootOptions={mode:"open"},_[z("elementProperties")]=new Map,_[z("finalized")]=new Map,q==null||q({ReactiveElement:_}),(v.reactiveElementVersions??(v.reactiveElementVersions=[])).push("2.1.2");/**
 * @license
 * Copyright 2017 Google LLC
 * SPDX-License-Identifier: BSD-3-Clause
 */const I=globalThis,Z=s=>s,M=I.trustedTypes,X=M?M.createPolicy("lit-html",{createHTML:s=>s}):void 0,ce="$lit$",f=`lit$${Math.random().toFixed(9).slice(2)}$`,le="?"+f,Ae=`<${le}>`,x=document,P=()=>x.createComment(""),O=s=>s===null||typeof s!="object"&&typeof s!="function",V=Array.isArray,ze=s=>V(s)||typeof(s==null?void 0:s[Symbol.iterator])=="function",D=`[ 	
\f\r]`,A=/<(?:(!--|\/[^a-zA-Z])|(\/?[a-zA-Z][^>\s]*)|(\/?$))/g,ee=/-->/g,te=/>/g,y=RegExp(`>|${D}(?:([^\\s"'>=/]+)(${D}*=${D}*(?:[^ 	
\f\r"'\`<>=]|("|')|))|$)`,"g"),ae=/'/g,re=/"/g,de=/^(?:script|style|textarea|title)$/i,Ie=s=>(e,...t)=>({_$litType$:s,strings:e,values:t}),l=Ie(1),C=Symbol.for("lit-noChange"),m=Symbol.for("lit-nothing"),oe=new WeakMap,b=x.createTreeWalker(x,129);function pe(s,e){if(!V(s)||!s.hasOwnProperty("raw"))throw Error("invalid template strings array");return X!==void 0?X.createHTML(e):e}const Pe=(s,e)=>{const t=s.length-1,a=[];let r,o=e===2?"<svg>":e===3?"<math>":"",i=A;for(let c=0;c<t;c++){const n=s[c];let p,h,d=-1,u=0;for(;u<n.length&&(i.lastIndex=u,h=i.exec(n),h!==null);)u=i.lastIndex,i===A?h[1]==="!--"?i=ee:h[1]!==void 0?i=te:h[2]!==void 0?(de.test(h[2])&&(r=RegExp("</"+h[2],"g")),i=y):h[3]!==void 0&&(i=y):i===y?h[0]===">"?(i=r??A,d=-1):h[1]===void 0?d=-2:(d=i.lastIndex-h[2].length,p=h[1],i=h[3]===void 0?y:h[3]==='"'?re:ae):i===re||i===ae?i=y:i===ee||i===te?i=A:(i=y,r=void 0);const g=i===y&&s[c+1].startsWith("/>")?" ":"";o+=i===A?n+Ae:d>=0?(a.push(p),n.slice(0,d)+ce+n.slice(d)+f+g):n+f+(d===-2?c:g)}return[pe(s,o+(s[t]||"<?>")+(e===2?"</svg>":e===3?"</math>":"")),a]};class R{constructor({strings:e,_$litType$:t},a){let r;this.parts=[];let o=0,i=0;const c=e.length-1,n=this.parts,[p,h]=Pe(e,t);if(this.el=R.createElement(p,a),b.currentNode=this.el.content,t===2||t===3){const d=this.el.content.firstChild;d.replaceWith(...d.childNodes)}for(;(r=b.nextNode())!==null&&n.length<c;){if(r.nodeType===1){if(r.hasAttributes())for(const d of r.getAttributeNames())if(d.endsWith(ce)){const u=h[i++],g=r.getAttribute(d).split(f),L=/([.?@])?(.*)/.exec(u);n.push({type:1,index:o,name:L[2],strings:g,ctor:L[1]==="."?Re:L[1]==="?"?Te:L[1]==="@"?Le:H}),r.removeAttribute(d)}else d.startsWith(f)&&(n.push({type:6,index:o}),r.removeAttribute(d));if(de.test(r.tagName)){const d=r.textContent.split(f),u=d.length-1;if(u>0){r.textContent=M?M.emptyScript:"";for(let g=0;g<u;g++)r.append(d[g],P()),b.nextNode(),n.push({type:2,index:++o});r.append(d[u],P())}}}else if(r.nodeType===8)if(r.data===le)n.push({type:2,index:o});else{let d=-1;for(;(d=r.data.indexOf(f,d+1))!==-1;)n.push({type:7,index:o}),d+=f.length-1}o++}}static createElement(e,t){const a=x.createElement("template");return a.innerHTML=e,a}}function $(s,e,t=s,a){var i,c;if(e===C)return e;let r=a!==void 0?(i=t._$Co)==null?void 0:i[a]:t._$Cl;const o=O(e)?void 0:e._$litDirective$;return(r==null?void 0:r.constructor)!==o&&((c=r==null?void 0:r._$AO)==null||c.call(r,!1),o===void 0?r=void 0:(r=new o(s),r._$AT(s,t,a)),a!==void 0?(t._$Co??(t._$Co=[]))[a]=r:t._$Cl=r),r!==void 0&&(e=$(s,r._$AS(s,e.values),r,a)),e}class Oe{constructor(e,t){this._$AV=[],this._$AN=void 0,this._$AD=e,this._$AM=t}get parentNode(){return this._$AM.parentNode}get _$AU(){return this._$AM._$AU}u(e){const{el:{content:t},parts:a}=this._$AD,r=((e==null?void 0:e.creationScope)??x).importNode(t,!0);b.currentNode=r;let o=b.nextNode(),i=0,c=0,n=a[0];for(;n!==void 0;){if(i===n.index){let p;n.type===2?p=new T(o,o.nextSibling,this,e):n.type===1?p=new n.ctor(o,n.name,n.strings,this,e):n.type===6&&(p=new Ue(o,this,e)),this._$AV.push(p),n=a[++c]}i!==(n==null?void 0:n.index)&&(o=b.nextNode(),i++)}return b.currentNode=x,r}p(e){let t=0;for(const a of this._$AV)a!==void 0&&(a.strings!==void 0?(a._$AI(e,a,t),t+=a.strings.length-2):a._$AI(e[t])),t++}}class T{get _$AU(){var e;return((e=this._$AM)==null?void 0:e._$AU)??this._$Cv}constructor(e,t,a,r){this.type=2,this._$AH=m,this._$AN=void 0,this._$AA=e,this._$AB=t,this._$AM=a,this.options=r,this._$Cv=(r==null?void 0:r.isConnected)??!0}get parentNode(){let e=this._$AA.parentNode;const t=this._$AM;return t!==void 0&&(e==null?void 0:e.nodeType)===11&&(e=t.parentNode),e}get startNode(){return this._$AA}get endNode(){return this._$AB}_$AI(e,t=this){e=$(this,e,t),O(e)?e===m||e==null||e===""?(this._$AH!==m&&this._$AR(),this._$AH=m):e!==this._$AH&&e!==C&&this._(e):e._$litType$!==void 0?this.$(e):e.nodeType!==void 0?this.T(e):ze(e)?this.k(e):this._(e)}O(e){return this._$AA.parentNode.insertBefore(e,this._$AB)}T(e){this._$AH!==e&&(this._$AR(),this._$AH=this.O(e))}_(e){this._$AH!==m&&O(this._$AH)?this._$AA.nextSibling.data=e:this.T(x.createTextNode(e)),this._$AH=e}$(e){var o;const{values:t,_$litType$:a}=e,r=typeof a=="number"?this._$AC(e):(a.el===void 0&&(a.el=R.createElement(pe(a.h,a.h[0]),this.options)),a);if(((o=this._$AH)==null?void 0:o._$AD)===r)this._$AH.p(t);else{const i=new Oe(r,this),c=i.u(this.options);i.p(t),this.T(c),this._$AH=i}}_$AC(e){let t=oe.get(e.strings);return t===void 0&&oe.set(e.strings,t=new R(e)),t}k(e){V(this._$AH)||(this._$AH=[],this._$AR());const t=this._$AH;let a,r=0;for(const o of e)r===t.length?t.push(a=new T(this.O(P()),this.O(P()),this,this.options)):a=t[r],a._$AI(o),r++;r<t.length&&(this._$AR(a&&a._$AB.nextSibling,r),t.length=r)}_$AR(e=this._$AA.nextSibling,t){var a;for((a=this._$AP)==null?void 0:a.call(this,!1,!0,t);e!==this._$AB;){const r=Z(e).nextSibling;Z(e).remove(),e=r}}setConnected(e){var t;this._$AM===void 0&&(this._$Cv=e,(t=this._$AP)==null||t.call(this,e))}}class H{get tagName(){return this.element.tagName}get _$AU(){return this._$AM._$AU}constructor(e,t,a,r,o){this.type=1,this._$AH=m,this._$AN=void 0,this.element=e,this.name=t,this._$AM=r,this.options=o,a.length>2||a[0]!==""||a[1]!==""?(this._$AH=Array(a.length-1).fill(new String),this.strings=a):this._$AH=m}_$AI(e,t=this,a,r){const o=this.strings;let i=!1;if(o===void 0)e=$(this,e,t,0),i=!O(e)||e!==this._$AH&&e!==C,i&&(this._$AH=e);else{const c=e;let n,p;for(e=o[0],n=0;n<o.length-1;n++)p=$(this,c[a+n],t,n),p===C&&(p=this._$AH[n]),i||(i=!O(p)||p!==this._$AH[n]),p===m?e=m:e!==m&&(e+=(p??"")+o[n+1]),this._$AH[n]=p}i&&!r&&this.j(e)}j(e){e===m?this.element.removeAttribute(this.name):this.element.setAttribute(this.name,e??"")}}class Re extends H{constructor(){super(...arguments),this.type=3}j(e){this.element[this.name]=e===m?void 0:e}}class Te extends H{constructor(){super(...arguments),this.type=4}j(e){this.element.toggleAttribute(this.name,!!e&&e!==m)}}class Le extends H{constructor(e,t,a,r,o){super(e,t,a,r,o),this.type=5}_$AI(e,t=this){if((e=$(this,e,t,0)??m)===C)return;const a=this._$AH,r=e===m&&a!==m||e.capture!==a.capture||e.once!==a.once||e.passive!==a.passive,o=e!==m&&(a===m||r);r&&this.element.removeEventListener(this.name,this,a),o&&this.element.addEventListener(this.name,this,e),this._$AH=e}handleEvent(e){var t;typeof this._$AH=="function"?this._$AH.call(((t=this.options)==null?void 0:t.host)??this.element,e):this._$AH.handleEvent(e)}}class Ue{constructor(e,t,a){this.element=e,this.type=6,this._$AN=void 0,this._$AM=t,this.options=a}get _$AU(){return this._$AM._$AU}_$AI(e){$(this,e)}}const N=I.litHtmlPolyfillSupport;N==null||N(R,T),(I.litHtmlVersions??(I.litHtmlVersions=[])).push("3.3.2");const Ee=(s,e,t)=>{const a=(t==null?void 0:t.renderBefore)??e;let r=a._$litPart$;if(r===void 0){const o=(t==null?void 0:t.renderBefore)??null;a._$litPart$=r=new T(e.insertBefore(P(),o),o,void 0,t??{})}return r._$AI(s),r};/**
 * @license
 * Copyright 2017 Google LLC
 * SPDX-License-Identifier: BSD-3-Clause
 */const w=globalThis;class S extends _{constructor(){super(...arguments),this.renderOptions={host:this},this._$Do=void 0}createRenderRoot(){var t;const e=super.createRenderRoot();return(t=this.renderOptions).renderBefore??(t.renderBefore=e.firstChild),e}update(e){const t=this.render();this.hasUpdated||(this.renderOptions.isConnected=this.isConnected),super.update(e),this._$Do=Ee(t,this.renderRoot,this.renderOptions)}connectedCallback(){var e;super.connectedCallback(),(e=this._$Do)==null||e.setConnected(!0)}disconnectedCallback(){var e;super.disconnectedCallback(),(e=this._$Do)==null||e.setConnected(!1)}render(){return C}}var se;S._$litElement$=!0,S.finalized=!0,(se=w.litElementHydrateSupport)==null||se.call(w,{LitElement:S});const W=w.litElementPolyfillSupport;W==null||W({LitElement:S});(w.litElementVersions??(w.litElementVersions=[])).push("4.2.2");function Me(){return"adoptedStyleSheets"in Document.prototype&&"replaceSync"in CSSStyleSheet.prototype}function k(s){if(!Me())return{sheet:null,text:s};const e=new CSSStyleSheet;return e.replaceSync(s),{sheet:e,text:s}}function me(s,e){const t=e.map(o=>o.sheet).filter(o=>o!==null);if(t.length===e.length){s.adoptedStyleSheets=t;return}const a="oatty-fallback-styles";let r=s.querySelector(`style[data-style-id='${a}']`);r||(r=document.createElement("style"),r.dataset.styleId=a,s.prepend(r)),r.textContent=e.map(o=>o.text).join(`
`)}const He='.m-docs-sidebar{position:sticky;top:5rem;display:grid;gap:var(--space-lg);align-content:start;border-right:1px solid rgb(76 86 106 / .45);padding-right:var(--space-md)}.m-docs-sidebar__group{display:grid;gap:.4rem}.m-docs-sidebar__title{color:var(--color-text-secondary);text-transform:uppercase;font-size:.72rem;letter-spacing:.08em}.m-docs-sidebar__link{color:var(--color-text-secondary);font-size:.9rem;border-radius:var(--radius-sm);padding:.25rem .4rem}.m-docs-sidebar__link.is-active{color:var(--color-text-primary);background:#5e81ac2e}.m-docs-header{display:grid;gap:.6rem}.m-docs-kicker{color:var(--color-accent);text-transform:uppercase;font-size:.72rem;letter-spacing:.09em;font-weight:700}.m-docs-title{font-size:clamp(2rem,4vw,2.7rem);line-height:var(--line-height-tight)}.m-docs-summary{color:var(--color-text-secondary);max-width:70ch}.m-summary-card{border:1px solid var(--color-surface-border);background:var(--gradient-brand-subtle);border-radius:var(--radius-lg);padding:var(--space-lg);display:grid;gap:var(--space-md)}.m-summary-card .m-summary-card__header{display:flex;justify-content:space-between;align-items:baseline;gap:var(--space-md)}.m-summary-card .m-summary-card__header span{color:var(--color-text-secondary);font-size:var(--font-size-sm)}.m-summary-card ul{display:grid;gap:.45rem;padding-left:1rem}.m-docs-section{display:grid;gap:var(--space-md);scroll-margin-top:6rem}.m-docs-section p{color:var(--color-text-secondary);margin:0}.m-docs-callout{border:1px solid var(--color-surface-border);border-radius:var(--radius-md);padding:var(--space-md);display:grid;gap:.35rem}.m-docs-callout h3{margin:0;font-size:var(--font-size-sm);text-transform:uppercase;letter-spacing:.06em}.m-docs-callout p{margin:0;color:var(--color-text-secondary);font-size:var(--font-size-sm)}.m-docs-callout__heading{display:flex;align-items:center;gap:.4rem}.m-docs-callout__icon{font-family:Material Symbols Outlined;font-weight:400;font-style:normal;font-size:1rem;line-height:1;letter-spacing:normal;text-transform:none;display:inline-block;white-space:nowrap;word-wrap:normal;direction:ltr;-webkit-font-feature-settings:"liga";-webkit-font-smoothing:antialiased;font-variation-settings:"FILL" 0,"wght" 400,"GRAD" 0,"opsz" 20}.m-docs-screenshot-trigger{display:block;width:100%;margin-top:var(--space-xs);padding:0;border:0;border-radius:var(--radius-sm);background:transparent;cursor:zoom-in}.m-docs-screenshot-image{width:100%;height:auto;display:block;border-radius:var(--radius-sm);border:1px solid var(--color-surface-border);box-shadow:var(--shadow-sm)}.m-docs-callout--screenshot{background:#88c0d017;border-color:#88c0d061}.m-docs-callout--fallback{background:#ebcb8b1a;border-color:#ebcb8b5c}.m-docs-callout--advanced{background:#a3be8c17;border-color:#a3be8c57}.m-docs-callout--tip{background:#d087701a;border-color:#d0877061}.m-docs-callout--expected{background:#b48ead1a;border-color:#b48ead5c}.m-docs-callout--recovery{background:#bf616a1a;border-color:#bf616a61}.m-docs-callout--generic{background:#5e81ac1a;border-color:#5e81ac57}.m-docs-feedback{border-top:1px solid var(--color-divider);margin-top:var(--space-xl);padding-top:var(--space-md)}.m-docs-feedback p{margin:0;color:var(--color-text-secondary);font-size:var(--font-size-sm)}.m-docs-pagination{display:flex;justify-content:space-between;gap:var(--space-md);border-top:1px solid var(--color-divider);padding-top:var(--space-xl)}.m-toc{position:sticky;top:5rem;display:grid;gap:.5rem;border-left:1px solid rgb(76 86 106 / .5);padding-left:.85rem}.m-toc .m-toc__title{margin:0 0 .2rem;color:var(--color-text-secondary);text-transform:uppercase;font-size:.72rem;letter-spacing:.09em}.m-toc a{color:var(--color-text-secondary);font-size:.82rem}.m-toc a:hover{color:var(--color-text-primary)}.m-toc__link--level-3{margin-left:.6rem}.m-toc__link--level-4{margin-left:1.2rem}.m-toc__link--level-5{margin-left:1.8rem}.m-toc__link--level-6{margin-left:2.4rem}@media(max-width:1100px){.m-docs-sidebar,.m-toc{display:none}}',he=k(He),qe='.m-logo{display:inline-flex;align-items:center;gap:var(--space-sm);font-weight:700;letter-spacing:.04em;text-decoration:none;color:var(--color-text-primary);transition:transform var(--transition-fast),opacity var(--transition-fast)}.m-logo:hover{opacity:.9;transform:translateY(-1px)}.m-logo img{display:block;width:2rem;height:2rem;filter:drop-shadow(0 2px 4px rgba(0,0,0,.3))}.m-logo .m-logo__glyph{display:grid;place-items:center;width:2rem;height:2rem;border-radius:var(--radius-sm);border:1px solid var(--color-accent-subtle);background:var(--color-accent-subtle);font-family:var(--font-mono);font-size:var(--font-size-sm);color:var(--color-accent)}.m-logo .m-logo__tag{font-size:var(--font-size-xs);color:var(--color-text-tertiary);border:1px solid var(--color-divider);border-radius:var(--radius-full);padding:.1rem .45rem}.m-skip-link{position:absolute;left:-9999px;top:var(--space-sm);z-index:100;border-radius:var(--radius-md);background:var(--color-elevated);border:1px solid var(--color-surface-border);padding:var(--space-sm) var(--space-md);color:var(--color-text-primary);text-decoration:none}.m-skip-link:focus{left:var(--space-sm)}.m-nav{display:inline-flex;align-items:center;gap:var(--space-lg)}.m-nav .m-nav__link{color:var(--color-text-secondary);font-size:var(--font-size-sm);text-decoration:none;transition:color var(--transition-fast);position:relative}.m-nav .m-nav__link:hover{color:var(--color-text-primary)}.m-nav .m-nav__link:hover:after{content:"";position:absolute;bottom:-.25rem;left:0;right:0;height:2px;background:var(--gradient-brand);border-radius:var(--radius-full)}.m-header-actions{display:inline-flex;align-items:center;gap:var(--space-md)}.m-search{display:inline-flex;align-items:center;gap:var(--space-sm);border:1px solid var(--color-divider);border-radius:var(--radius-md);padding:.5rem var(--space-md);color:var(--color-text-secondary);background:var(--color-surface);font-size:var(--font-size-sm);text-decoration:none;transition:all var(--transition-fast)}.m-search:hover{border-color:var(--color-surface-border);background:var(--color-elevated)}.m-search kbd{border:1px solid var(--color-divider);border-radius:var(--radius-sm);padding:.08rem .32rem;font-family:var(--font-mono);font-size:var(--font-size-xs);color:var(--color-text-primary);background:var(--color-background-alt)}.m-section{display:grid;gap:1rem;padding-top:.3rem}.m-section+.m-section{border-top:1px solid rgb(76 86 106 / .32);padding-top:1.9rem}.m-section__title{font-size:clamp(1.35rem,2vw,1.95rem);letter-spacing:.01em}.m-hero{display:grid;grid-template-columns:1.12fr .88fr;gap:1.1rem;align-items:stretch}.m-hero .m-hero__content{display:grid;gap:1rem;border:1px solid var(--color-surface-border);border-radius:.8rem;background:#3b425273;padding:clamp(1rem,2vw,1.5rem)}.m-hero .m-hero__headline{font-size:clamp(2rem,5.2vw,3.2rem);line-height:1.02;max-width:20ch;text-wrap:balance}.m-hero .m-hero__summary{max-width:65ch;color:var(--color-text-secondary);font-size:clamp(1.02rem,2.1vw,1.15rem);line-height:1.55}.m-hero .m-hero__subtext{color:var(--color-text-secondary);font-size:.92rem}.m-hero .m-hero__actions{display:flex;flex-wrap:wrap;gap:.7rem}.m-terminal-demo{border:1px solid var(--color-surface-border);border-radius:.8rem;background:linear-gradient(180deg,#242933f2,#242933bf);padding:1rem;display:grid;gap:.8rem}.m-terminal-demo .m-terminal-demo__title{color:var(--color-text-secondary);font-size:.76rem;text-transform:uppercase;letter-spacing:.08em}.m-terminal-demo .m-terminal-demo__window{border-radius:.65rem;border:1px solid rgb(76 86 106 / .8);background:#1f242d;padding:.9rem;min-height:260px;display:grid;gap:.8rem;align-content:start;overflow:hidden}.m-terminal-demo .m-terminal-demo__line{margin:0;font-family:Iosevka Term,JetBrains Mono,ui-monospace,monospace;color:#d8dee9;font-size:.82rem;white-space:nowrap;overflow:hidden;border-right:2px solid #88c0d0;animation:type-line 4.8s steps(70,end) infinite}.m-terminal-demo .m-terminal-demo__line--accent{color:#a3be8c;animation-delay:1.2s}@keyframes type-line{0%{max-width:0;opacity:0}10%{opacity:1}70%{max-width:100%;opacity:1}90%{max-width:100%;opacity:.9}to{max-width:0;opacity:0}}.m-button{display:inline-flex;align-items:center;justify-content:center;gap:var(--space-sm);border:1px solid var(--color-surface-border);border-radius:var(--radius-md);padding:.625rem var(--space-lg);font-weight:600;font-size:var(--font-size-sm);text-decoration:none;cursor:pointer;transition:all var(--transition-fast);background:var(--color-surface);color:var(--color-text-primary)}.m-button:hover{transform:translateY(-1px);border-color:var(--color-accent);box-shadow:var(--shadow-md)}.m-button:active{transform:translateY(0)}.m-button--primary{background:var(--gradient-brand);color:var(--color-background);border-color:transparent;font-weight:700}.m-button--primary:hover{box-shadow:var(--shadow-glow);opacity:.95}.m-card{border:1px solid var(--color-divider);background:var(--color-surface);border-radius:var(--radius-lg);padding:var(--space-xl);display:flex;flex-direction:column;gap:var(--space-md);transition:all var(--transition-base)}.m-card:hover{border-color:var(--color-surface-border);box-shadow:var(--shadow-md)}.m-card .m-card__title{font-size:var(--font-size-lg);font-weight:600;color:var(--color-text-primary)}.m-card .m-card__text{color:var(--color-text-secondary);line-height:var(--line-height-relaxed);font-size:var(--font-size-base)}.m-card--visual{align-content:start}.m-list{margin:0;padding-left:1.2rem;display:grid;gap:.6rem;color:var(--color-text-secondary);line-height:1.5}.m-visual__caption{margin:0;color:var(--color-text-secondary);font-size:.77rem;text-transform:uppercase;letter-spacing:.08em}.m-cli-pile{display:grid;gap:.5rem}.m-cli-pile span{border:1px dashed var(--color-surface-border);border-radius:.5rem;padding:.45rem .55rem;color:var(--color-text-secondary);font-family:Iosevka Term,JetBrains Mono,ui-monospace,monospace;font-size:.82rem}.m-visual__arrow{text-align:center;color:var(--color-accent);font-weight:700;margin:.2rem 0}.m-one-tool{border:1px solid rgb(136 192 208 / .72);background:#88c0d021;border-radius:.6rem;padding:.65rem .7rem;text-align:center;font-weight:700}.m-principle{align-content:start}.m-principle__icon{width:1.65rem;height:1.65rem;border:1px solid var(--color-surface-border);border-radius:.35rem;display:grid;place-items:center;color:var(--color-accent);font-weight:700;background:#88c0d014;font-size:.84rem}.m-micro-example{margin:0;color:var(--color-text-secondary);font-size:.82rem}.m-micro-example code{color:var(--color-text-primary);font-family:Iosevka Term,JetBrains Mono,ui-monospace,monospace}.m-section--callout{padding:clamp(.8rem,2vw,1.2rem);border:1px solid rgb(136 192 208 / .4);border-radius:.72rem;background:#81a1c114}.m-video-placeholder{min-height:240px;border-radius:.8rem;border:1px dashed rgb(136 192 208 / .9);background:linear-gradient(180deg,#242933e6,#1f242df2);display:grid;place-items:center;color:#88c0d0;font-weight:700;letter-spacing:.03em}.m-feature{align-content:start}.m-media-placeholder{min-height:140px;border-radius:.64rem;border:1px dashed var(--color-surface-border);background:#242933e6;display:grid;place-items:center;color:var(--color-text-secondary);font-size:.78rem;text-transform:uppercase;letter-spacing:.07em}.m-steps{position:relative}.m-step{align-content:center;min-height:126px}.m-step .m-step__number{margin:0;color:var(--color-accent);font-weight:700;font-family:Iosevka Term,JetBrains Mono,ui-monospace,monospace;font-size:.87rem}.m-quote{gap:.75rem}.m-code{margin:0;overflow:auto;border-radius:var(--radius-md);border:1px solid var(--color-divider);background:var(--color-background-alt);padding:var(--space-lg);font-family:var(--font-mono);font-size:var(--font-size-sm);line-height:var(--line-height-relaxed);color:var(--color-text-secondary);box-shadow:var(--shadow-sm)}.m-code code{font-family:inherit;color:var(--color-text-primary)}.m-code--hero{margin-top:var(--space-md);background:var(--color-background)}.m-badges{display:flex;flex-wrap:wrap;gap:.8rem}.m-badges img{max-width:100%;height:auto}.m-footer__links{display:flex;flex-wrap:wrap;gap:1rem;color:var(--color-text-secondary);font-size:.9rem}@media(max-width:980px){.m-hero{grid-template-columns:minmax(0,1fr)}}.m-screenshot{cursor:pointer;transition:transform var(--transition-base),opacity var(--transition-base)}.m-screenshot:hover{transform:scale(1.02);opacity:.9}.m-lightbox{display:none;position:fixed;top:0;left:0;right:0;bottom:0;background:#000000eb;z-index:1000;cursor:pointer;align-items:center;justify-content:center;padding:var(--space-lg)}.m-lightbox.is-open{display:flex}.m-lightbox img{max-width:95vw;max-height:95vh;width:auto;height:auto;border-radius:var(--radius-lg);box-shadow:0 20px 60px #0009;cursor:default}.m-lightbox .m-lightbox__close{position:absolute;top:var(--space-lg);right:var(--space-lg);width:3rem;height:3rem;border-radius:var(--radius-full);background:#ffffff1a;border:1px solid rgba(255,255,255,.2);color:#fff;font-size:var(--font-size-2xl);display:grid;place-items:center;cursor:pointer;transition:all var(--transition-fast)}.m-lightbox .m-lightbox__close:hover{background:#fff3;transform:scale(1.1)}@media(max-width:740px){.m-nav{width:100%;justify-content:flex-start;flex-wrap:wrap}.m-header-actions{width:100%;justify-content:flex-start}}',ue=k(qe),De=':host,:root,:root[data-theme=system],:root[data-theme=light],:root[data-theme=dark],:root[data-theme=high-contrast]{color-scheme:dark;--color-background: #1a1f2e;--color-background-alt: #151922;--color-surface: #242933;--color-elevated: #2e3440;--color-surface-border: #3b4252;--color-divider: rgba(236, 239, 244, .08);--color-text-primary: #eceff4;--color-text-secondary: #d8dee9;--color-text-tertiary: #88929f;--color-text-muted: #616e88;--color-accent: #88c0d0;--color-accent-strong: #81a1c1;--color-accent-hover: #5e81ac;--color-accent-subtle: rgba(136, 192, 208, .12);--color-focus: #ebcb8b;--color-good: #a3be8c;--color-warning: #ebcb8b;--color-bad: #bf616a;--color-info: #88c0d0;--gradient-brand: linear-gradient(135deg, #88c0d0 0%, #81a1c1 50%, #5e81ac 100%);--gradient-brand-subtle: linear-gradient(135deg, rgba(136, 192, 208, .2) 0%, rgba(129, 161, 193, .15) 100%);--surface-glow: radial-gradient(1200px circle at 15% -5%, rgba(136, 192, 208, .15), transparent 55%), radial-gradient(900px circle at 85% 10%, rgba(129, 161, 193, .12), transparent 50%), linear-gradient(180deg, #1a1f2e 0%, #12161f 100%);--hero-glow: radial-gradient(800px circle at 50% -20%, rgba(136, 192, 208, .2), transparent 60%), radial-gradient(600px circle at 80% 30%, rgba(129, 161, 193, .15), transparent 55%);--shadow-sm: 0 1px 2px 0 rgba(0, 0, 0, .3);--shadow-md: 0 4px 6px -1px rgba(0, 0, 0, .4), 0 2px 4px -1px rgba(0, 0, 0, .3);--shadow-lg: 0 10px 15px -3px rgba(0, 0, 0, .5), 0 4px 6px -2px rgba(0, 0, 0, .4);--shadow-xl: 0 20px 25px -5px rgba(0, 0, 0, .5), 0 10px 10px -5px rgba(0, 0, 0, .4);--shadow-glow: 0 0 20px rgba(136, 192, 208, .3);--space-xs: .25rem;--space-sm: .5rem;--space-md: 1rem;--space-lg: 1.5rem;--space-xl: 2rem;--space-2xl: 3rem;--space-3xl: 4rem;--space-4xl: 6rem;--radius-sm: .25rem;--radius-md: .5rem;--radius-lg: .75rem;--radius-xl: 1rem;--radius-full: 9999px;--font-sans: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, "Helvetica Neue", Arial, sans-serif;--font-mono: "SF Mono", Monaco, "Cascadia Code", "Roboto Mono", Consolas, "Courier New", monospace;--font-size-xs: .75rem;--font-size-sm: .875rem;--font-size-base: 1rem;--font-size-lg: 1.125rem;--font-size-xl: 1.25rem;--font-size-2xl: 1.5rem;--font-size-3xl: 1.875rem;--font-size-4xl: 2.25rem;--font-size-5xl: 3rem;--line-height-tight: 1.25;--line-height-normal: 1.5;--line-height-relaxed: 1.75;--transition-fast: .15s cubic-bezier(.4, 0, .2, 1);--transition-base: .25s cubic-bezier(.4, 0, .2, 1);--transition-slow: .35s cubic-bezier(.4, 0, .2, 1);--content-max-width: 80ch;--page-max-width: 1280px;--nav-height: 4rem}',ge=k(De),Ne=".u-flow>*+*{margin-top:var(--space-md)}.u-visually-hidden{position:absolute;width:1px;height:1px;margin:-1px;padding:0;border:0;overflow:hidden;clip:rect(0 0 0 0);white-space:nowrap}.u-text-muted{color:var(--color-text-secondary)}.u-mono{font-family:var(--font-mono)}",fe=k(Ne),Y=class Y extends S{createRenderRoot(){return this.attachShadow({mode:"open"})}connectedCallback(){super.connectedCallback(),this.shadowRoot&&me(this.shadowRoot,[ge,fe,ue,he])}openLightbox(e){e.imageSrc&&this.dispatchEvent(new CustomEvent("docs-open-lightbox",{detail:{src:e.imageSrc,alt:e.imageAlt??this.calloutLabel(e)},bubbles:!0,composed:!0}))}calloutLabel(e){var t;if((t=e.label)!=null&&t.trim())return e.label;switch(e.type){case"expected":return"Expected Result";case"recovery":return"If this fails";case"screenshot":return"Screenshot Target";case"fallback":return"CLI Fallback";case"advanced":return"Advanced";case"tip":return"Tip";default:return"Note"}}calloutIcon(e){switch(e.type){case"expected":return"check_circle";case"recovery":return"error";case"screenshot":return"image";case"fallback":return"terminal";case"advanced":return"psychology_alt";case"tip":return"tips_and_updates";default:return"info"}}calloutClass(e){return`m-docs-callout ${new Set(["expected","recovery","screenshot","fallback","advanced","tip"]).has(e.type)?`m-docs-callout--${e.type}`:"m-docs-callout--generic"}`}sectionHeadingLevel(e){return e.headingLevel??2}renderSectionHeading(e){switch(this.sectionHeadingLevel(e)){case 3:return l`<h3>${e.title}</h3>`;case 4:return l`<h4>${e.title}</h4>`;case 5:return l`<h5>${e.title}</h5>`;case 6:return l`<h6>${e.title}</h6>`;case 2:default:return l`<h2>${e.title}</h2>`}}scrollToSection(e){var r;const t=(r=this.shadowRoot)==null?void 0:r.getElementById(e);if(!t)return!1;const a=window.matchMedia("(prefers-reduced-motion: reduce)").matches;return t.scrollIntoView({behavior:a?"auto":"smooth",block:"start"}),!0}render(){return this.page?l`
            <header class="m-docs-header">
                <p class="m-docs-kicker">Docs</p>
                <h1 class="m-docs-title">${this.page.title}</h1>
                <p class="m-docs-summary">${this.page.summary}</p>
            </header>

            <section class="m-summary-card" aria-label="What you'll learn">
                <div class="m-summary-card__header">
                    <h2>What you'll learn</h2>
                    <span>${this.page.estimatedTime}</span>
                </div>
                <ul>
                    ${this.page.learnBullets.map(e=>l`
                        <li>${e}</li>`)}
                </ul>
            </section>

            ${this.page.sections.map(e=>l`
                        <section id="${e.id}" class="m-docs-section">
                            ${this.renderSectionHeading(e)}
                            ${e.paragraphs.map(t=>l`<p>${t}</p>`)}
                            ${e.codeSample?l`
                                <pre class="m-code"><code>${e.codeSample}</code></pre>`:""}
                            ${(e.callouts??[]).map(t=>l`
                                        <aside class="${this.calloutClass(t)}"
                                               aria-label="${this.calloutLabel(t)}">
                                            <h3 class="m-docs-callout__heading">
                                                <span class="material-symbols-outlined m-docs-callout__icon"
                                                      aria-hidden="true">${this.calloutIcon(t)}</span>
                                                <span>${this.calloutLabel(t)}</span>
                                            </h3>
                                            <p>${t.content}</p>
                                            ${t.imageSrc?l`
                                                        <button
                                                                type="button"
                                                                class="m-docs-screenshot-trigger"
                                                                @click="${()=>this.openLightbox(t)}"
                                                                aria-label="Open screenshot in lightbox"
                                                        >
                                                            <img class="m-docs-screenshot-image"
                                                                 src="${t.imageSrc}"
                                                                 alt="${t.imageAlt??this.calloutLabel(t)}"/>
                                                        </button>
                                                    `:""}
                                        </aside>
                                    `)}
                        </section>
                    `)}

            ${this.page.feedbackPrompt?l`
                        <footer class="m-docs-feedback">
                            <p>${this.page.feedbackPrompt}</p>
                        </footer>
                    `:""}
        `:l``}};Y.properties={page:{attribute:!1}};let F=Y;customElements.define("docs-page-view",F);const We={path:"/docs/learn/getting-oriented",title:"Getting Oriented",summary:"Learn the core interaction model so you can navigate Oatty quickly and recover from common UI friction.",learnBullets:["Move focus predictably with keyboard and mouse.","Use logs, hints, and help affordances during execution.","Recognize layout changes as terminal width changes.","Keep a stable mental model across views and modals."],estimatedTime:"8-12 min",feedbackPrompt:"Was this page helpful? Rate it or suggest improvements in docs feedback.",sections:[{id:"prerequisites",title:"Prerequisites",paragraphs:["Launch the TUI with `oatty`.","Use a terminal size that shows navigation, content, and hints clearly.","Confirm your keyboard sends Tab, Shift+Tab, and Esc correctly."],codeSample:"oatty",callouts:[{type:"expected",content:"The default TUI view opens with visible focus and hints."},{type:"recovery",content:"If rendering clips, resize the terminal and relaunch. If key input is inconsistent, verify terminal key settings."},{type:"screenshot",imageSrc:"/Oatty-finder.png",imageAlt:"Oatty UI screenshot",content:"Capture the default landing view with focus outline visible."}]},{id:"navigation-model",title:"Navigation Model",paragraphs:["Use the left navigation to switch top-level views.","Treat each view as a focused workspace with shared interaction rules.","Return to the same view repeatedly to build speed."],callouts:[{type:"expected",content:"You can move between Library, Run Command, Find, Workflows, Plugins, and MCP Server without confusion."},{type:"recovery",content:"If a view does not react to input, cycle focus with Tab until the target region highlights."},{type:"screenshot",imageSrc:"/Oatty-finder.png",imageAlt:"Oatty UI screenshot",content:"Capture left navigation with one selected view and one hovered view."},{type:"advanced",content:"Most views use Tab and Shift+Tab as the primary focus-cycle pattern."}]},{id:"keyboard-focus",title:"Keyboard and Focus",paragraphs:["Press Tab to move focus forward.","Press Shift+Tab to move focus backward.","When a list is focused, press Up and Down to move one row.","When a long list is focused, press PgUp and PgDown to move faster.","List navigation keys are focus-scoped, and the hints bar remains the source of truth for active view behavior.","Press Esc to close modals and transient overlays."],callouts:[{type:"expected",content:"Focusable areas highlight consistently, and modal dismissal works with Esc."},{type:"recovery",content:"If focus appears stuck, close overlays with Esc, then cycle Tab until the intended element gains focus."},{type:"screenshot",imageSrc:"/Oatty-finder.png",imageAlt:"Oatty UI screenshot",content:"Capture two states showing focus moved across different interactive regions."},{type:"advanced",content:"Hint spans show context-sensitive actions; base focus movement is intentionally omitted from hints in many views."}]},{id:"mouse-interaction",title:"Mouse Interaction",paragraphs:["Click list rows to select entries.","Click buttons to trigger the same action exposed through keyboard controls.","Use mouse selection for quick scanning and keyboard for repetitive execution."],callouts:[{type:"expected",content:"Clicking an interactive element updates focus and action state predictably."},{type:"recovery",content:"If clicks do not act on the expected element, click once to focus the panel, then click the target action again."},{type:"screenshot",imageSrc:"/Oatty-finder.png",imageAlt:"Oatty UI screenshot",content:"Capture a list selection state and a button click state in the same view."},{type:"advanced",content:"Some modal flows intentionally close through mouse affordances; Esc remains the global close behavior."}]},{id:"logs-panel",title:"Logs and Inspection",paragraphs:["Toggle logs with Ctrl+L.","Use logs to verify command/workflow status and inspect failures.","Filter and inspect entries before rerunning actions."],callouts:[{type:"expected",content:"The logs panel opens and closes without losing your current workflow context."},{type:"recovery",content:"If no entries appear, execute a command first. If the panel feels unresponsive, refocus it with Tab before filtering."},{type:"screenshot",imageSrc:"/Oatty-finder.png",imageAlt:"Oatty UI screenshot",content:"Capture logs closed and logs open with one selected log entry."},{type:"fallback",content:"For non-interactive automation logs, run commands with CLI output and collect logs in your shell/CI system."},{type:"advanced",content:"Layout can place logs differently at wider terminal sizes while preserving the same interaction model."}]},{id:"help-affordances",title:"Hints and Help",paragraphs:["Read the hints bar before executing an unfamiliar action.","Use in-view help affordances to confirm expected key and mouse behavior.","Treat hints as the fastest way to recover from uncertainty."],callouts:[{type:"expected",content:"You can identify available actions in the active view without leaving the screen."},{type:"recovery",content:"If hints do not match behavior, confirm the active focus area. Hints are context-sensitive."},{type:"screenshot",imageSrc:"/Oatty-finder.png",imageAlt:"Oatty UI screenshot",content:"Capture the hints bar while focus is on a list, then on an action button."},{type:"advanced",content:"Parent components may own shared hotkeys in specific flows; this is an intentional exception pattern."}]},{id:"layout-responsiveness",title:"Terminal Size and Layout Changes",paragraphs:["Watch panel arrangement as terminal width changes.","Keep enough width for side panels when doing multi-pane inspection.","Resize before starting long tasks to avoid context shifts mid-run."],callouts:[{type:"expected",content:"You can predict where panels move as width changes and keep critical context visible."},{type:"recovery",content:"If a panel appears missing, increase width or cycle views to restore expected layout."},{type:"screenshot",imageSrc:"/Oatty-finder.png",imageAlt:"Oatty UI screenshot",content:"Capture a narrow and a wide terminal state for the same view."},{type:"advanced",content:"At wider widths, right-side panel strategies can change which border edge acts as the resize affordance."}]},{id:"next-steps",title:"Next Steps",paragraphs:["Continue to Search and Run Commands for deeper execution flow.","Then move to Library and Workflows to build repeatable operations."],callouts:[{type:"expected",content:"You can navigate the TUI confidently and continue through feature modules faster."},{type:"screenshot",imageSrc:"/Oatty-finder.png",imageAlt:"Oatty UI screenshot",content:"Capture the final oriented state with a selected view and visible hints."}]}]},Be={path:"/docs/learn/how-oatty-executes-safely",title:"How Oatty Executes Safely",summary:"Understand the trust model: suggestion, preview, validation, and explicit operator control before execution.",learnBullets:["Connect Oatty MCP tooling to your AI assistant for controlled planning support.","Separate suggestion from execution so generated plans stay reviewable.","Use preview and validation outputs before running commands or workflows.","Interpret failures quickly and recover with deterministic next steps.","Keep manual control even when using AI assistants and natural language requests."],estimatedTime:"6-9 min",feedbackPrompt:"Was this page helpful? Rate it or suggest improvements in docs feedback.",sections:[{id:"trust-model",title:"Trust Model at a Glance",paragraphs:["Connect Oatty to your AI assistant through MCP so planning and execution tools are discoverable in one place.","Oatty treats natural language as a planning input, not an execution bypass.","Suggested commands or workflows are surfaced for operator review before run.","Execution remains explicit and observable through status, logs, and result views."],callouts:[{type:"tip",content:"Use this mental model: suggest -> inspect -> validate -> run."},{type:"screenshot",imageSrc:"/Oatty-run.png",imageAlt:"Run command view with output and logs",content:"Capture a run view that shows selected command, output, and logs together."}]},{id:"preview-validation",title:"Preview and Validation Before Run",paragraphs:["Before execution, confirm command arguments or workflow inputs match intent.","Use preview and validation tools to catch schema, dependency, and input issues early.","When validation fails, use returned violations and suggested actions to repair quickly."],callouts:[{type:"expected",content:"Validation failures are specific enough to fix in one edit cycle."},{type:"recovery",content:"If a step fails validation, correct the reported field/dependency and run validation again before execution."},{type:"advanced",content:"For workflows, `workflow.resolve_inputs` is the best pre-run readiness checkpoint."}]},{id:"operator-control",title:"Operator Control and Manual Overrides",paragraphs:["Assisted planning does not remove manual operation paths.","You can still run commands directly, edit workflow manifests, and provide inputs manually.","This keeps behavior deterministic and audit-friendly in high-stakes changes."],callouts:[{type:"tip",content:"For production changes, prefer explicit command/workflow review over one-shot execution."},{type:"fallback",content:"Use CLI commands for explicit scriptable execution when you need non-interactive control."}]},{id:"failure-recovery",title:"Failure and Recovery Pattern",paragraphs:["Treat failures as structured feedback, not dead ends.","Read the first actionable error, update the relevant input/spec, and rerun the smallest validation step first.","Then rerun execution once readiness checks pass."],callouts:[{type:"expected",content:"Most failures should map to a clear next action from validation or error metadata."},{type:"screenshot",imageSrc:"/docs-screenshot-placeholder.svg",imageAlt:"Failure and recovery screenshot placeholder",content:"Capture a single recovery flow: failure signal -> correction -> successful rerun."}]},{id:"next-steps",title:"Next Steps",paragraphs:["Continue to Search and Run Commands for command-level execution patterns.","Continue to Workflows Basics for input collection, run controls, and step-level inspection.","Use workflow export/import flows to share reviewed workflows with teammates and CI pipelines."],callouts:[{type:"expected",content:"You can evaluate assisted suggestions without sacrificing operator control."}]}]},Fe={path:"/docs/learn/library-and-catalogs",title:"Library and Catalogs",summary:"Manage catalogs as your command source of truth, including import, enablement, base URLs, and removal.",learnBullets:["Import catalogs into the shared command registry.","Toggle catalog enablement and verify active state.","Define request headers such as Authorization in the headers editor.","Set, add, and remove base URLs safely.","Use CLI fallback for catalog import automation."],estimatedTime:"10-14 min",feedbackPrompt:"Was this page helpful? Rate it or suggest improvements in docs feedback.",sections:[{id:"prerequisites",title:"Prerequisites",paragraphs:["Open the Library view.","Prepare an OpenAPI source path or URL for import."],callouts:[{type:"expected",content:"Library opens with catalog list and catalog details panels available."},{type:"recovery",content:"If Library is empty, import a catalog first."},{type:"screenshot",imageSrc:"/Oatty-library.png",imageAlt:"Library catalog screenshot",content:"Capture Library with list, details, and base URL areas visible."}]},{id:"import-catalog",title:"Step 1: Import a Catalog",paragraphs:["Select Import in Library.","Provide the schema source and complete the import flow.","Verify the new catalog appears in the list."],callouts:[{type:"expected",content:"A new catalog entry appears and is selectable in Library."},{type:"recovery",content:"If import fails, validate schema format and source path/URL, then retry."},{type:"screenshot",imageSrc:"/Oatty-library.png",imageAlt:"Library catalog screenshot",content:"Capture successful import with the new catalog selected."},{type:"fallback",content:"CLI fallback: `oatty import <path-or-url> --kind catalog`."},{type:"advanced",content:"Imported catalogs become a shared command surface used by TUI, workflows, CLI, and MCP tooling."}]},{id:"toggle-enablement",title:"Step 2: Toggle Catalog Enablement",paragraphs:["Focus the catalog list and select a catalog.","Toggle enabled state from the list action path.","Confirm status changes in the catalog row and details."],callouts:[{type:"expected",content:"Catalog status updates between enabled and disabled."},{type:"recovery",content:"If status does not change, ensure the catalog row is focused before toggling."},{type:"screenshot",imageSrc:"/Oatty-library.png",imageAlt:"Library catalog screenshot",content:"Capture enabled and disabled states for the same catalog."}]},{id:"headers-management",title:"Step 3: Define Request Headers",paragraphs:["Open the catalog headers editor in Library.","Add header key-value entries, including `Authorization` when required.","Leave header values empty only when your API contract allows optional values."],callouts:[{type:"expected",content:"Header rows are visible in the catalog details and persist after focus changes."},{type:"recovery",content:"If headers do not persist, check for invalid/empty header keys and correct the row."},{type:"screenshot",imageSrc:"/Oatty-library.png",imageAlt:"Library catalog screenshot",content:"Capture headers editor with a valid Authorization header row."},{type:"advanced",content:"Header validation enforces non-empty header keys before saving."}]},{id:"base-url-management",title:"Step 4: Manage Base URLs",paragraphs:["Select a catalog and open its base URL section.","Set the active base URL from the URL list.","Add or remove base URLs as needed."],callouts:[{type:"expected",content:"One base URL is marked active and list updates reflect add/remove actions."},{type:"recovery",content:"If updates fail validation, correct the base URL value and retry."},{type:"screenshot",imageSrc:"/Oatty-library.png",imageAlt:"Library catalog screenshot",content:"Capture base URL list with active selection and add/remove controls."},{type:"advanced",content:"Base URL validation rejects invalid or empty URL sets for a catalog."}]},{id:"remove-catalog",title:"Step 5: Remove a Catalog",paragraphs:["Select the catalog to remove.","Trigger Remove and confirm in the modal.","Verify the catalog no longer appears in the list."],callouts:[{type:"expected",content:"Selected catalog is removed from Library."},{type:"recovery",content:"If remove is disabled, select a catalog row first."},{type:"screenshot",imageSrc:"/Oatty-library.png",imageAlt:"Library catalog screenshot",content:"Capture remove confirmation modal and post-remove list state."},{type:"advanced",content:"Removal is destructive. Use confirmation flow to prevent accidental deletion."}]},{id:"next-steps",title:"Next Steps",paragraphs:["Continue to Workflows Basics to run imported workflows with structured inputs.","Return to Search and Run Commands to validate command behavior against updated catalogs."],callouts:[{type:"expected",content:"You can keep catalog state aligned with your execution workflows."},{type:"screenshot",imageSrc:"/Oatty-library.png",imageAlt:"Library catalog screenshot",content:"Capture final Library state with stable catalog and base URL configuration."}]}]},je={path:"/docs/learn/mcp-http-server",title:"MCP HTTP Server",summary:"Start the local MCP HTTP server, verify endpoint details, and configure clients to connect reliably.",learnBullets:["Start and stop the server from the TUI control view.","Use active endpoint and client count details for verification.","Configure MCP clients with the running `/mcp` endpoint.","Use auto-start when you need server availability on TUI launch."],estimatedTime:"8-12 min",feedbackPrompt:"Was this page helpful? Rate it or suggest improvements in docs feedback.",sections:[{id:"prerequisites",title:"Prerequisites",paragraphs:["Open MCP HTTP Server view in the TUI.","Confirm local network policy allows loopback access."],callouts:[{type:"expected",content:"Server controls and status details are visible."},{type:"recovery",content:"If the view is unavailable, switch to MCP HTTP Server from left navigation."},{type:"screenshot",imageSrc:"/Oatty-mcp-server.png",imageAlt:"MCP server screenshot",content:"Capture MCP HTTP Server panel with status and controls."}]},{id:"start-stop-server",title:"Step 1: Start and Stop the Server",paragraphs:["Use Start to launch the MCP HTTP server.","Use Stop to shut down the server when needed.","Read status changes to confirm lifecycle transitions."],callouts:[{type:"expected",content:"Status transitions between Stopped, Starting, Running, and Stopping as actions execute."},{type:"recovery",content:"If start fails, review Last error and retry from a stopped state."},{type:"screenshot",imageSrc:"/Oatty-mcp-server.png",imageAlt:"MCP server screenshot",content:"Capture Start action, Running status, and Stop control state."}]},{id:"endpoint-details",title:"Step 2: Verify Endpoint Details",paragraphs:["Read Configured bind and Active endpoint in the details panel.","Use Active endpoint as the canonical client connection target while running.","Monitor Connected clients to confirm successful inbound sessions."],callouts:[{type:"expected",content:"Active endpoint displays `http://<bound-address>/mcp` while running."},{type:"recovery",content:"If Active endpoint is `not running`, start the server and verify status returns Running."},{type:"screenshot",imageSrc:"/Oatty-mcp-server.png",imageAlt:"MCP server screenshot",content:"Capture details panel showing configured bind, active endpoint, and connected clients."},{type:"advanced",content:"Default bind is loopback (`127.0.0.1:62889`) unless overridden by config."}]},{id:"configure-clients",title:"Step 3: Configure MCP Clients to Connect",paragraphs:["Set client server URL/base URL to the Active endpoint shown in TUI.","Use the exact `/mcp` path from Active endpoint.","Restart or reconnect the client after updating configuration."],codeSample:`# Example client target from TUI details
http://127.0.0.1:62889/mcp`,callouts:[{type:"expected",content:"Connected clients count increases after client connection succeeds."},{type:"recovery",content:"If clients cannot connect, verify server is Running, endpoint includes `/mcp`, and client points to loopback host/port."},{type:"screenshot",imageSrc:"/Oatty-mcp-server.png",imageAlt:"MCP server screenshot",label:"Screenshot Target: Server View",content:"Capture server details showing Active endpoint and connected clients."},{type:"screenshot",imageSrc:"/Oatty-mcp-server.png",imageAlt:"MCP server screenshot",label:"Screenshot Target: Client Config",content:"Capture client configuration that matches the exact Active endpoint including `/mcp`."},{type:"advanced",content:"Use local bind addresses for local clients. Keep endpoint and client config synchronized when bind settings change."}]},{id:"auto-start",title:"Step 4: Configure Auto-start",paragraphs:["Toggle Auto-start when you want the server started with TUI launch.","Leave Auto-start disabled for manual lifecycle control."],callouts:[{type:"expected",content:"Auto-start toggle state persists and reflects your runtime preference."},{type:"recovery",content:"If toggle does not persist, retry toggle and confirm no configuration write errors are logged."},{type:"screenshot",imageSrc:"/Oatty-mcp-server.png",imageAlt:"MCP server screenshot",content:"Capture Auto-start enabled and disabled states."}]},{id:"next-steps",title:"Next Steps",paragraphs:["Return to Plugins to validate tool-level integrations exposed through connected clients.","Continue to Reference docs for configuration and environment variable details."],callouts:[{type:"expected",content:"You can run a stable local MCP server and connect clients without ambiguity."},{type:"screenshot",imageSrc:"/Oatty-mcp-server.png",imageAlt:"MCP server screenshot",content:"Capture final running state with non-zero connected clients."}]}]},Ve={path:"/docs/learn/plugins",title:"Plugins",summary:"Manage plugin lifecycle and configuration from one TUI workflow, including details, validation, and save paths.",learnBullets:["Inspect plugin inventory and open plugin details.","Start, stop, and restart plugins from table and details contexts.","Validate and save plugin editor updates safely.","Define remote headers or local env vars in editor key-value rows."],estimatedTime:"10-14 min",feedbackPrompt:"Was this page helpful? Rate it or suggest improvements in docs feedback.",sections:[{id:"prerequisites",title:"Prerequisites",paragraphs:["Open the Plugins view.","Confirm at least one plugin entry is present."],callouts:[{type:"expected",content:"Plugin table loads with search, list, and action controls."},{type:"recovery",content:"If no plugins are listed, create or import plugin definitions first."},{type:"screenshot",imageSrc:"/docs-screenshot-placeholder.svg",imageAlt:"Screenshot placeholder",content:"Capture plugin table with selected row and visible actions."}]},{id:"plugin-table-operations",title:"Step 1: Use Plugin Table Operations",paragraphs:["Select a plugin row from the table.","Open details from the selected plugin.","Use start, stop, and restart actions from the table-level controls."],callouts:[{type:"expected",content:"Selected plugin actions execute and status updates reflect control operations."},{type:"recovery",content:"If actions are unavailable, verify a plugin row is selected and supports the target action."},{type:"screenshot",imageSrc:"/docs-screenshot-placeholder.svg",imageAlt:"Screenshot placeholder",content:"Capture selected plugin row, details open action, and start/stop controls."},{type:"advanced",content:"Details, edit, and start/stop/restart are available through dedicated hotkeys in focused table context."}]},{id:"plugin-details",title:"Step 2: Inspect Plugin Details",paragraphs:["Open plugin details from the selected row.","Review metadata, logs, and exposed tool information.","Run control operations from details when needed."],callouts:[{type:"expected",content:"Details modal loads plugin metadata and tool/log sections for the selected plugin."},{type:"recovery",content:"If details fail to load, refresh details and verify plugin is still selected."},{type:"screenshot",imageSrc:"/docs-screenshot-placeholder.svg",imageAlt:"Screenshot placeholder",content:"Capture plugin details modal with loaded data and control hints visible."},{type:"advanced",content:"Details includes explicit error rendering when detail loading fails."}]},{id:"plugin-editor",title:"Step 3: Use Plugin Editor Validate and Save",paragraphs:["Open plugin editor for add/edit flows.","Validate configuration before save.","Save only after required fields and validation pass."],callouts:[{type:"expected",content:"Validation feedback is shown, and save persists valid plugin configuration."},{type:"recovery",content:"If save is disabled, resolve validation issues and required fields first."},{type:"screenshot",imageSrc:"/docs-screenshot-placeholder.svg",imageAlt:"Screenshot placeholder",content:"Capture editor form, validation state, and enabled save button."},{type:"advanced",content:"Validate and Save availability is focus-aware and state-dependent."}]},{id:"plugin-config-headers-env",title:"Step 4: Define Headers or Env Vars",paragraphs:["Use Remote transport to define request headers in the key-value editor.","Use Local transport to define environment variables in the same editor.","Add required auth values such as `Authorization` for remote integrations when needed."],callouts:[{type:"expected",content:"Key-value rows persist and match the selected transport mode."},{type:"recovery",content:"If configuration fails validation, correct invalid or empty keys and validate again."},{type:"screenshot",imageSrc:"/docs-screenshot-placeholder.svg",imageAlt:"Screenshot placeholder",content:"Capture plugin editor showing Remote headers with an Authorization row."},{type:"advanced",content:"The key-value editor label switches by transport: Headers for Remote, Env Vars for Local."}]},{id:"next-steps",title:"Next Steps",paragraphs:["Continue to MCP HTTP Server to expose Oatty tools over a local MCP endpoint.","Return to Workflows Basics to combine plugin-backed tools with workflow execution."],callouts:[{type:"expected",content:"You can operate plugin lifecycle and configuration with predictable outcomes."},{type:"screenshot",imageSrc:"/docs-screenshot-placeholder.svg",imageAlt:"Screenshot placeholder",content:"Capture final plugin operational state with clear status indicators."}]}]},Ye={path:"/docs/quick-start",title:"Quick Start",summary:"Start in the TUI, run a real command, run a workflow, then use CLI fallback for automation.",learnBullets:["Use the TUI layout, focus movement, and help affordances with confidence.","Import a catalog and discover commands through the interactive TUI path.","Run a command and verify results in the logs and result views.","Run a workflow end-to-end with structured inputs and step status."],estimatedTime:"10-15 min",feedbackPrompt:"Was this page helpful? Rate it or suggest improvements in docs feedback.",sections:[{id:"install",title:"Install Oatty",paragraphs:["Install Oatty with npm for the fastest setup.","Run `oatty --help` to verify the install."],codeSample:`npm install -g oatty
oatty --help`,callouts:[{type:"expected",content:"The `oatty --help` command prints usage and exits successfully."},{type:"recovery",content:"If the command is missing, restart the shell and check your PATH. For source builds, run from the release binary path."},{type:"fallback",label:"Alternative installation",content:"If npm is unavailable, build from source: `cargo build --release` and run `./target/release/oatty`."}]},{id:"launch",title:"Launch the TUI",paragraphs:["Launch the interface with `oatty` in your terminal.","Identify the left navigation, main content pane, and the hints bar."],codeSample:"oatty",callouts:[{type:"expected",content:"The TUI opens with visible navigation and an empty Library view."},{type:"screenshot",label:"Default TUI landing state",imageSrc:"/assets/quick-start/oatty-first-launch.png",imageAlt:"Oatty TUI landing view with empty Library view",content:"Default TUI landing state with left nav and hints bar visible"},{type:"recovery",content:"If the UI does not render correctly, increase terminal size and relaunch. If colors are unreadable, verify your terminal supports 256 colors."},{type:"advanced",content:"Power-user affordances: `Ctrl+L` toggles logs and `Ctrl+T` opens the theme picker when enabled."}]},{id:"import_schema",title:"Import Your First Catalog",paragraphs:["Open the Library and look for the Import button. Tab until it's focused then press Enter, space bar or click with your mouse."],callouts:[{type:"screenshot",label:"Import file/URL picker",imageSrc:"/assets/quick-start/oatty-import.png",imageAlt:"Oatty TUI import file picker with OpenAPI v3 schema selected from filesystem",content:"Import file picker with OpenAPI v3 schema selected from filesystem"},{type:"expected",content:"Oatty allows you to browse your filesystem or paste a URL and hit Enter or click the Open button to import."},{type:"recovery",content:"If import fails, verify the schema path/URL and format. Oatty currently supports OpenAPI v3 in both yaml and json formats. Retry import from Library, or run the CLI fallback to inspect errors."},{type:"fallback",content:"CLI import fallback: `oatty import <path-or-url> --kind catalog` (supports path and HTTP/HTTPS URL)."}]},{id:"optional_command_prefix",headingLevel:3,title:"Optional Command Prefix",paragraphs:["An optional custom command prefix dialog will appear after choosing a file or pasting a URL. This allows you to customize the command prefix for the imported catalog, which can be useful for organizing commands or avoiding conflicts with existing commands.","Skipping this step will derive the prefix from the schema directly."],callouts:[{type:"expected",content:"An optional custom command prefix dialog will appear allowing you to customize the command prefix for the imported catalog."},{type:"screenshot",label:"Custom command prefix dialog",imageSrc:"/assets/quick-start/oatty-optional-prefix.png",imageAlt:"Optional custom command prefix dialog presented after importing an OpenAPI v3 schema",content:"Command prefix customization dialog"},{type:"expected",content:"The Library view updates with the imported catalog and shows the summary of what was imported."},{type:"recovery",content:"If the custom prefix you enter is incorrect, you must remove the catalog and retry the import."}]},{id:"complete_import",headingLevel:3,title:"Complete Import",paragraphs:["The import process will complete after you press Enter or click the Open button. The Library view will update with the imported catalog and populate the details panel."],callouts:[{type:"screenshot",label:"Library view after import",imageSrc:"/assets/quick-start/oatty-library-with-catalog.png",imageAlt:"Oatty TUI library view with the newly imported catalog",content:"Library view with the imported catalog and a populated details panel."},{type:"recovery",content:"If the import fails, verify the schema path/URL and format. Oatty currently supports OpenAPI v3 in both yaml and json formats."},{type:"advanced",content:"Advanced flow: add/remove catalogs and manage base URLs in Library; this is covered in Learn: Library and Catalogs.",label:"Configuration management"}]},{id:"run-command",title:"Discover and Run a Command",paragraphs:["Open Run Command and type a search phrase then press the Tab key to see matching result","Select a command, use Tab to see available flags and arg, input values and execute.","Inspect structured output and logs in the UI."],callouts:[{type:"expected",content:"Command execution completes and results/logs show the final status."},{type:"recovery",content:"If no command appears, confirm a catalog is imported. If execution fails, open command help and verify required inputs."},{type:"screenshot",imageSrc:"/Oatty-run.png",imageAlt:"Run Command view showing execution output and logs",content:"Shows the command runner with completion list open and a second shot showing executed result/log output."},{type:"fallback",content:"CLI fallback for the same action: run the selected command directly with required flags/args."},{type:"recovery",label:"Command help",content:"Read the help (Ctrl+h) for the command to understand required inputs and flags. Verify the Auth header is configured and correct"},{type:"advanced",content:"For deeper discovery, use the Find/Browser view to inspect commands and send selected entries back to the runner.",label:"Advanced discovery"}]}]},Ge={path:"/docs/learn/search-and-run-commands",title:"Search and Run Commands",summary:"Use the TUI command flow to find commands quickly, execute with confidence, and inspect results without leaving the interface.",learnBullets:["Run the primary TUI search-to-execution path.","Use command help and hints before execution.","Use Find browser to inspect and hand off commands.","Use CLI fallback for automation and scripts."],estimatedTime:"10-14 min",feedbackPrompt:"Was this page helpful? Rate it or suggest improvements in docs feedback.",sections:[{id:"prerequisites",title:"Prerequisites",paragraphs:["Launch Oatty with `oatty`.","Import at least one catalog so commands are discoverable.","Keep logs available for execution verification."],codeSample:"oatty",callouts:[{type:"expected",content:"You can open Run Command and see command suggestions from your imported catalog."},{type:"recovery",content:"If no commands appear, import a catalog first in Library."},{type:"screenshot",imageSrc:"/Oatty-run.png",imageAlt:"Run command screenshot",content:"Capture Run Command focused with an empty input and visible hints."},{type:"fallback",content:"Catalog import fallback: `oatty import <path-or-url> --kind catalog`."}]},{id:"open-run-command",title:"Step 1: Open Run Command",paragraphs:["Navigate to Run Command from the left navigation.","Start typing your task phrase to query commands."],callouts:[{type:"expected",content:"The command input is focused automatically and ready for text entry."},{type:"recovery",content:"If typing does not update input, press Tab until the input area is focused."},{type:"screenshot",imageSrc:"/Oatty-run.png",imageAlt:"Run command screenshot",content:"Capture Run Command with focused input and active cursor."}]},{id:"search-and-select",title:"Step 2: Search and Select a Command",paragraphs:["Type a task phrase such as `create app`.","Use Up and Down to change selection in the suggestion list.","Confirm the selected command before executing."],callouts:[{type:"expected",content:"A relevant command is selected in the suggestion list."},{type:"recovery",content:"If search returns nothing, verify catalog import and try broader search terms."},{type:"screenshot",imageSrc:"/Oatty-run.png",imageAlt:"Run command screenshot",content:"Capture suggestion list open with one highlighted command."},{type:"advanced",content:"Selection behavior is focus-scoped. Hints remain the source of truth for active controls."}]},{id:"review-help",title:"Step 3: Review Help Before Running",paragraphs:["Open command help from the active command context.","Verify required inputs and expected command shape.","Return to input and complete required values."],callouts:[{type:"expected",content:"Required inputs are known before execution."},{type:"recovery",content:"If help is unavailable, switch focus to the command area and read the hints bar for supported actions."},{type:"screenshot",imageSrc:"/Oatty-run.png",imageAlt:"Run command screenshot",content:"Capture command help visible with required input details."},{type:"advanced",content:"Use this step to prevent avoidable execution failures from missing required arguments."}]},{id:"execute-command",title:"Step 4: Execute and Inspect Output",paragraphs:["Run the selected command from the command runner.","Inspect structured output in the result view.","Open logs to verify completion or debug failures."],callouts:[{type:"expected",content:"Execution reaches a terminal status, and output/logs show the final result."},{type:"recovery",content:"If execution fails, read the first actionable log message, adjust required inputs, and rerun."},{type:"screenshot",imageSrc:"/Oatty-run.png",imageAlt:"Run command screenshot",content:"Capture executed result state and a selected log entry tied to the run."},{type:"fallback",content:"Run the same command in CLI for scripts and CI with explicit flags and arguments."}]},{id:"find-browser-handoff",title:"Step 5: Use Find Browser for Discovery",paragraphs:["Open Find to browse commands with summaries and categories.","Select a command and send it to Run Command.","Execute from Run Command after reviewing inputs."],callouts:[{type:"expected",content:"A command selected in Find appears in Run Command ready for execution."},{type:"recovery",content:"If handoff does not occur, confirm focus is in Find and retry the handoff action shown in hints."},{type:"screenshot",imageSrc:"/Oatty-run.png",imageAlt:"Run command screenshot",content:"Capture Find browser with selected command and the post-handoff Run Command state."},{type:"advanced",content:"Find is best for exploration; Run Command is optimized for fast execution loops."}]},{id:"cli-fallback",title:"CLI Fallback for Automation",paragraphs:["Use CLI search when you need non-interactive discovery.","Run commands directly in scripts and CI with explicit inputs."],codeSample:`oatty search "create app"
oatty apps create --name demo-app`,callouts:[{type:"expected",content:"You can execute the same command path outside the TUI."},{type:"recovery",content:"If CLI execution differs from TUI expectation, verify command arguments and active catalog configuration."},{type:"advanced",content:"Use TUI for discovery and validation first, then promote stable command lines into automation."}]},{id:"next-steps",title:"Next Steps",paragraphs:["Continue to Library and Catalogs to manage command sources.","Then continue to Workflows to compose repeatable multi-step execution."],callouts:[{type:"expected",content:"You can discover and run commands reliably in both TUI and CLI contexts."},{type:"screenshot",imageSrc:"/Oatty-run.png",imageAlt:"Run command screenshot",content:"Capture a completed run state that includes selected command, output, and logs."}]}]},Ke={path:"/docs/learn/workflows-basics",title:"Workflows Basics",summary:"Move from workflow selection to input collection to execution, then control active runs from the run view.",learnBullets:["Import and remove workflows from the workflow list.","Open pre-run inputs and resolve required values.","Run workflows and inspect step status and details.","Use pause/resume/cancel controls during active runs."],estimatedTime:"12-16 min",feedbackPrompt:"Was this page helpful? Rate it or suggest improvements in docs feedback.",sections:[{id:"prerequisites",title:"Prerequisites",paragraphs:["Open Workflows view.","Ensure at least one workflow exists in the list."],callouts:[{type:"expected",content:"Workflow list is visible with selectable rows and action buttons."},{type:"recovery",content:"If the list is empty, import a workflow."},{type:"screenshot",imageSrc:"/Oatty-workflows-runner.png",imageAlt:"Workflow execution screenshot",content:"Capture workflow list with one selected row and action buttons."},{type:"fallback",content:"CLI fallback import: `oatty import <path-or-url> --kind workflow`."}]},{id:"manage-list",title:"Step 1: Manage Workflow List",paragraphs:["Use Import to add workflows to the list.","Select a workflow and use Remove when needed.","Use search and list navigation to locate workflows quickly."],callouts:[{type:"expected",content:"Workflow list reflects import/remove actions and selection state."},{type:"recovery",content:"If Remove is unavailable, select a workflow first."},{type:"screenshot",imageSrc:"/Oatty-workflows-runner.png",imageAlt:"Workflow execution screenshot",content:"Capture import action and remove confirmation flow."},{type:"advanced",content:"List navigation supports row movement and page jumps for larger workflow sets."}]},{id:"open-inputs",title:"Step 2: Open Inputs and Collect Values",paragraphs:["Press Enter on a selected workflow to open inputs.","Review required fields and collect values from provider or manual entry paths.","Use manual entry when provider selection is not appropriate."],callouts:[{type:"expected",content:"Required inputs are resolved and Run becomes available."},{type:"recovery",content:"If input collection blocks progress, fill missing required values and retry."},{type:"screenshot",imageSrc:"/Oatty-workflows-runner.png",imageAlt:"Workflow execution screenshot",content:"Capture input list, details panel, and collector/manual entry paths."},{type:"advanced",content:"Provider-backed inputs depend on declared dependencies; unresolved upstream values block provider execution."}]},{id:"start-run",title:"Step 3: Start a Workflow Run",paragraphs:["Run from the input view after required values are set.","Move to the run view and monitor step transitions.","Open step detail and logs for verification."],callouts:[{type:"expected",content:"Run view shows workflow status and step-level execution progress."},{type:"recovery",content:"If run fails early, inspect the first failing step detail and log message before rerunning."},{type:"screenshot",imageSrc:"/Oatty-workflows-runner.png",imageAlt:"Workflow execution screenshot",content:"Capture active run view with step table, detail action, and log linkage."},{type:"fallback",content:"CLI fallback: `oatty workflow list`, `oatty workflow preview <id>`, `oatty workflow run <id> --input key=value`."}]},{id:"run-controls",title:"Step 4: Control Active Runs",paragraphs:["Use Pause or Resume based on current run state.","Use Cancel when you need to stop execution.","Use Done to close completed runs."],callouts:[{type:"expected",content:"Run control actions update run state and status messaging."},{type:"recovery",content:"If a control is disabled, verify the current run state supports that action."},{type:"screenshot",imageSrc:"/Oatty-workflows-runner.png",imageAlt:"Workflow execution screenshot",content:"Capture pause/resume/cancel controls across different run states."},{type:"advanced",content:"Known limitation: step-level rerun/resume is not yet first-class."}]},{id:"next-steps",title:"Next Steps",paragraphs:["Continue to Plugins to integrate plugin-backed tools used by workflows.","Then continue to MCP HTTP Server to expose Oatty tools for MCP clients.","Return to Search and Run Commands to validate command-level behavior used inside workflows."],callouts:[{type:"expected",content:"You can execute workflows repeatedly with predictable input and control behavior."},{type:"screenshot",imageSrc:"/Oatty-workflows-runner.png",imageAlt:"Workflow execution screenshot",content:"Capture a completed run with terminal status and finalized step table."}]}]},U=[Ye,Be,We,Ge,Fe,Ke,Ve,je],Qe=":host{display:block;min-height:100vh;font-family:var(--font-sans);background:var(--surface-glow);color:var(--color-text-primary);line-height:var(--line-height-normal)}*{box-sizing:border-box;margin:0;padding:0}:where(h1,h2,h3,h4,h5,h6){margin:0;font-weight:700;line-height:var(--line-height-tight);color:var(--color-text-primary)}:where(p){margin:0;line-height:var(--line-height-relaxed)}:where(a){color:var(--color-accent);text-decoration:none;transition:color var(--transition-fast)}:where(a:hover){color:var(--color-accent-strong)}:where(button,select){font:inherit;color:inherit;background:none;border:none;cursor:pointer}:where(button,a,select):focus-visible{outline:2px solid var(--color-focus);outline-offset:2px;border-radius:var(--radius-sm)}:where(ul,ol){list-style-position:inside}:where(code){font-family:var(--font-mono);font-size:.9em}@keyframes blink{0%,50%{opacity:1}51%,to{opacity:0}}@media(prefers-reduced-motion:reduce){*,*:before,*:after{animation-duration:1ms!important;animation-iteration-count:1!important;scroll-behavior:auto!important;transition-duration:1ms!important}}",Je=k(Qe),Ze=".l-container{width:min(var(--page-max-width),calc(100% - var(--space-xl)));margin-inline:auto;padding-inline:var(--space-md)}.l-shell{width:min(var(--page-max-width),calc(100% - var(--space-xl)));margin-inline:auto}.l-header{position:sticky;top:0;z-index:50;-webkit-backdrop-filter:blur(12px) saturate(180%);backdrop-filter:blur(12px) saturate(180%);background:#1a1f2ed9;border-bottom:1px solid var(--color-divider);height:var(--nav-height)}.l-header .l-header__inner{display:flex;align-items:center;justify-content:space-between;gap:var(--space-lg);height:100%}.l-main{min-height:calc(100vh - var(--nav-height));padding:var(--space-4xl) 0}.l-hero{display:grid;gap:var(--space-3xl);padding:var(--space-4xl) 0;background:var(--hero-glow)}.l-hero__content{max-width:65ch;margin:0 auto;text-align:center}.l-section{padding:var(--space-4xl) 0}.l-section+.l-section{border-top:1px solid var(--color-divider)}.l-grid{display:grid;gap:var(--space-lg)}.l-grid--two{grid-template-columns:repeat(auto-fit,minmax(min(100%,320px),1fr))}.l-grid--three{grid-template-columns:repeat(auto-fit,minmax(min(100%,280px),1fr))}.l-grid--four{grid-template-columns:repeat(auto-fit,minmax(min(100%,240px),1fr))}.l-grid--features{grid-template-columns:repeat(auto-fit,minmax(min(100%,320px),1fr));gap:var(--space-xl)}.l-split{display:grid;grid-template-columns:minmax(0,1fr) 280px;gap:var(--space-3xl);align-items:start}.l-docs-layout{display:grid;grid-template-columns:240px minmax(0,1fr) 240px;gap:var(--space-3xl);align-items:start}.l-docs-layout__content{display:flex;flex-direction:column;gap:var(--space-3xl);max-width:var(--content-max-width)}.l-footer{border-top:1px solid var(--color-divider);padding:var(--space-3xl) 0;margin-top:var(--space-4xl)}.l-stack{display:flex;flex-direction:column;gap:var(--space-md)}.l-stack--sm{gap:var(--space-sm)}.l-stack--lg{gap:var(--space-lg)}.l-stack--xl{gap:var(--space-xl)}.l-flex{display:flex;gap:var(--space-md)}.l-flex--center{align-items:center;justify-content:center}.l-flex--between{align-items:center;justify-content:space-between}.l-flex--wrap{flex-wrap:wrap}@media(max-width:1024px){.l-docs-layout{grid-template-columns:minmax(0,1fr) 220px}.l-split{grid-template-columns:minmax(0,1fr)}}@media(max-width:768px){.l-container,.l-shell{width:calc(100% - var(--space-lg))}.l-main,.l-hero,.l-section{padding:var(--space-2xl) 0}.l-docs-layout{grid-template-columns:minmax(0,1fr)}.l-grid--features{gap:var(--space-lg)}}@media(max-width:640px){.l-header .l-header__inner{gap:var(--space-md)}.l-hero,.l-section{padding:var(--space-xl) 0}}",Xe=k(Ze),et=".is-muted{color:var(--color-text-secondary)}.is-hidden{display:none!important}",tt=k(et),at=[{section:"Get Started",links:[{title:"Quick Start",path:"/docs/quick-start"}]},{section:"Learn",links:[{title:"How Oatty Executes Safely",path:"/docs/learn/how-oatty-executes-safely"},{title:"Getting Oriented",path:"/docs/learn/getting-oriented"},{title:"Search and Run Commands",path:"/docs/learn/search-and-run-commands"},{title:"Library and Catalogs",path:"/docs/learn/library-and-catalogs"},{title:"Workflows Basics",path:"/docs/learn/workflows-basics"},{title:"Plugins",path:"/docs/learn/plugins"},{title:"MCP HTTP Server",path:"/docs/learn/mcp-http-server"}]},{section:"Guides",links:[{title:"Run First Workflow",path:"/docs/guides/run-first-workflow"},{title:"Provider-backed Inputs",path:"/docs/guides/provider-backed-inputs"}]},{section:"Reference",links:[{title:"CLI Commands",path:"/docs/reference/cli-commands"},{title:"TUI Interactions",path:"/docs/reference/tui-interactions"}]}];class rt extends S{constructor(){super(...arguments),this.currentPath=this.normalizePath(window.location.pathname),this.onPopState=()=>{this.currentPath=this.normalizePath(window.location.pathname),this.requestUpdate()},this.navigate=e=>{const t=e.currentTarget,a=t==null?void 0:t.getAttribute("href");!a||a.startsWith("http")||a.startsWith("#")||(e.preventDefault(),history.pushState({},"",a),this.currentPath=this.normalizePath(window.location.pathname),window.scrollTo({top:0}),this.requestUpdate())}}createRenderRoot(){return this.attachShadow({mode:"open"})}connectedCallback(){super.connectedCallback(),this.shadowRoot&&me(this.shadowRoot,[Je,ge,fe,Xe,ue,tt,he]),window.addEventListener("popstate",this.onPopState)}disconnectedCallback(){window.removeEventListener("popstate",this.onPopState),super.disconnectedCallback()}openLightbox(e,t){var o;const a=(o=this.shadowRoot)==null?void 0:o.querySelector(".m-lightbox"),r=a==null?void 0:a.querySelector("img");a&&r&&(r.setAttribute("src",e),r.setAttribute("alt",t),a.classList.add("is-open"))}closeLightbox(){var t;const e=(t=this.shadowRoot)==null?void 0:t.querySelector(".m-lightbox");e&&e.classList.remove("is-open")}handleDocsOpenLightbox(e){var t;(t=e.detail)!=null&&t.src&&this.openLightbox(e.detail.src,e.detail.alt??"Screenshot")}renderLightbox(){return l`
      <div class="m-lightbox" @click="${this.closeLightbox}">
        <div class="m-lightbox__close" aria-label="Close lightbox">✕</div>
        <img src="" alt="" @click="${e=>e.stopPropagation()}" />
      </div>
    `}normalizePath(e){if(!e||e==="/")return"/";const t=e.replace(/\/+$/,"");return t==="/docs"?"/docs/quick-start":t}isDocsRoute(){return this.currentPath.startsWith("/docs")}handleTableOfContentsClick(e,t){var o,i;e.preventDefault();const a=(o=this.shadowRoot)==null?void 0:o.querySelector("docs-page-view");(((i=a==null?void 0:a.scrollToSection)==null?void 0:i.call(a,t))??!1)&&history.replaceState({},"",`${this.currentPath}#${t}`)}currentDocsPage(){return U.find(e=>e.path===this.currentPath)}docsNeighborPages(){const e=U.findIndex(t=>t.path===this.currentPath);return e<0?{previous:void 0,next:void 0}:{previous:U[e-1],next:U[e+1]}}renderDocs(){const e=this.currentDocsPage();if(!e)return l`
        <a class="m-skip-link" href="#main-content">Skip to content</a>
        <header class="l-header">
          <div class="l-shell l-header__inner">
            <a href="/" @click="${this.navigate}" class="m-logo" aria-label="Oatty home">
              <img src="/logo-icon.svg" alt="Oatty logo" style="width: 2rem; height: 2rem;" />
              <span style="font-size: var(--font-size-lg); font-weight: 700; letter-spacing: 0.05em;">OATTY</span>
            </a>
            <div class="m-header-actions">
              <a class="m-button" href="/" @click="${this.navigate}">Back to Home</a>
            </div>
          </div>
        </header>
        <main id="main-content" class="l-main">
          <div class="l-shell">
            <article class="m-card">
              <h1 class="m-docs-title">Docs page not found</h1>
              <p class="m-card__text">This docs route is not implemented yet.</p>
              <a class="m-button m-button--primary" href="/docs/quick-start" @click="${this.navigate}">Go to Quick Start</a>
            </article>
          </div>
        </main>
      `;const{previous:t,next:a}=this.docsNeighborPages();return l`
      <a class="m-skip-link" href="#main-content">Skip to content</a>
      <header class="l-header">
        <div class="l-shell l-header__inner">
          <a href="/" @click="${this.navigate}" class="m-logo" aria-label="Oatty home">
            <img src="/logo-icon.svg" alt="Oatty logo" style="width: 2rem; height: 2rem;" />
            <span style="font-size: var(--font-size-lg); font-weight: 700; letter-spacing: 0.05em;">OATTY</span>
          </a>
          <nav class="m-nav" aria-label="Primary">
            <a class="m-nav__link" href="/docs/quick-start" @click="${this.navigate}">Quick Start</a>
            <a class="m-nav__link" href="/" @click="${this.navigate}">Home</a>
            <a class="m-nav__link" href="https://github.com/oattyio/oatty" target="_blank" rel="noopener">GitHub</a>
          </nav>
        </div>
      </header>

      <main id="main-content" class="l-main">
        <div class="l-shell l-docs-layout">
          <aside class="m-docs-sidebar" aria-label="Docs navigation">
            ${at.map(r=>l`
                <section class="m-docs-sidebar__group">
                  <h2 class="m-docs-sidebar__title">${r.section}</h2>
                  ${r.links.map(o=>{const i=o.path===this.currentPath;return l`<a class="m-docs-sidebar__link ${i?"is-active":""}" href="${o.path}" @click="${this.navigate}"
                      >${o.title}</a
                    >`})}
                </section>
              `)}
          </aside>

          <article class="l-docs-layout__content">
            <docs-page-view .page=${e} @docs-open-lightbox=${this.handleDocsOpenLightbox}></docs-page-view>

            <nav class="m-docs-pagination" aria-label="Page navigation">
              ${t?l`<a class="m-button" href="${t.path}" @click="${this.navigate}">← ${t.title}</a>`:l`<span></span>`}
              ${a?l`<a class="m-button m-button--primary" href="${a.path}" @click="${this.navigate}">${a.title} →</a>`:l``}
            </nav>
          </article>

          <aside class="m-toc" aria-label="On this page">
            <p class="m-toc__title">On this page</p>
            ${e.sections.map(r=>l`<a
                  class="m-toc__link m-toc__link--level-${r.headingLevel??2}"
                  href="#${r.id}"
                  @click="${o=>this.handleTableOfContentsClick(o,r.id)}"
                  >${r.title}</a
                >`)}
          </aside>
        </div>
      </main>
    `}render(){const e=this.renderLightbox();return this.isDocsRoute()?l`${e}${this.renderDocs()}`:l`
      ${e}
      <a class="m-skip-link" href="#main-content">Skip to content</a>

      <header class="l-header">
        <div class="l-shell l-header__inner">
          <a href="#" class="m-logo" aria-label="Oatty home">
            <img src="/logo-icon.svg" alt="Oatty logo" style="width: 2rem; height: 2rem;" />
            <span style="font-size: var(--font-size-lg); font-weight: 700; letter-spacing: 0.05em;">OATTY</span>
          </a>
          <nav class="m-nav" aria-label="Primary">
            <a class="m-nav__link" href="#problem">Problem</a>
            <a class="m-nav__link" href="#how-it-works">How It Works</a>
            <a class="m-nav__link" href="#features">Features</a>
            <a class="m-nav__link" href="#install">Install</a>
          </nav>
          <div class="m-header-actions">
            <a class="m-button m-button--primary" href="/docs/quick-start" @click="${this.navigate}">Start Quick Start</a>
          </div>
        </div>
      </header>

      <main id="main-content">
        <!-- Hero Section -->
        <section class="l-hero">
          <div class="l-shell">
            <div class="l-hero__content">
              <img src="/logo-lockup.svg" alt="Oatty - Schema-driven CLI+TUI+MCP" style="width: min(600px, 90%); height: auto; margin: 0 auto var(--space-2xl); display: block; filter: drop-shadow(0 4px 12px rgba(0, 0, 0, 0.3));" />
              <h1 style="font-size: var(--font-size-4xl); font-weight: 700; line-height: var(--line-height-tight); margin: 0; text-wrap: balance;">
                Natural language assistance for multi-vendor API operations, with safety built in
              </h1>
              <p style="font-size: var(--font-size-xl); color: var(--color-text-secondary); line-height: var(--line-height-relaxed); margin: var(--space-lg) 0;">
                Import schemas from the services you use. Describe your goal. Oatty suggests commands and workflows, then you preview, validate, and confirm before anything runs.
              </p>
              <div class="l-flex" style="justify-content: center; margin-top: var(--space-xl);">
                <a class="m-button m-button--primary" href="#how-it-works">Watch Demo</a>
                <a class="m-button" href="/docs/quick-start" @click="${this.navigate}">Start with Quick Start</a>
                <a class="m-button" href="https://github.com/oattyio/oatty" target="_blank" rel="noopener">View on GitHub</a>
              </div>
              <article class="m-card" style="max-width: 840px; margin: var(--space-2xl) auto 0; text-align: left; border: 1px dashed var(--color-accent); background: var(--color-background-alt);">
                <h2 style="font-size: var(--font-size-lg); margin-bottom: var(--space-sm);">Hero Demo Placeholder</h2>
                <p class="m-card__text" style="margin-bottom: var(--space-sm);">
                  Replace this with a short looping demo: prompt → suggestions → preview/validation → confirmation → logs.
                </p>
                <p style="font-family: var(--font-mono); font-size: var(--font-size-sm); color: var(--color-text-secondary); margin: 0;">
                  /public/demo-hero.mp4 (optional: /public/demo-hero.webm)
                </p>
              </article>
              <pre class="m-code m-code--hero" style="max-width: 600px; margin: var(--space-2xl) auto 0;"><code>npm install -g oatty

# Start in TUI (recommended)
oatty

# Use CLI fallback for automation
oatty search "create app"
oatty apps create --name demo-app</code></pre>
            </div>
          </div>
        </section>

        <!-- How It Works -->
        <section id="how-it-works" class="l-section">
          <div class="l-shell">
            <p style="font-size: var(--font-size-sm); text-transform: uppercase; letter-spacing: 0.1em; color: var(--color-accent); margin-bottom: var(--space-md); font-weight: 700;">How It Works</p>
            <h2 style="font-size: var(--font-size-4xl); font-weight: 700; margin: 0 0 var(--space-2xl);">
              Trust-first execution in three steps
            </h2>
            <div style="display: grid; grid-template-columns: repeat(auto-fit, minmax(min(100%, 280px), 1fr)); gap: var(--space-lg); margin-bottom: var(--space-2xl);">
              <article class="m-card">
                <h3 class="m-card__title">1. Connect Oatty to your AI assistant</h3>
                <p class="m-card__text">Use Oatty MCP server tooling so your assistant can discover and call Oatty tools.</p>
              </article>
              <article class="m-card">
                <h3 class="m-card__title">2. Describe your goal</h3>
                <p class="m-card__text">Your assistant can import needed OpenAPI catalogs, discover commands, and draft a cross-vendor workflow.</p>
              </article>
              <article class="m-card">
                <h3 class="m-card__title">3. Review, execute, and share</h3>
                <p class="m-card__text">Validate the plan, confirm execution, then save and share workflows for teammates and CI usage.</p>
              </article>
            </div>
            <article class="m-card" style="background: var(--gradient-brand-subtle); border: 1px solid var(--color-accent);">
              <h3 style="font-size: var(--font-size-2xl); margin-bottom: var(--space-md);">Migration Walkthrough Placeholder</h3>
              <p class="m-card__text" style="margin-bottom: var(--space-sm);">
                Example storyline: "Move my Postgres database and app from Vercel to Render" with suggestion preview and user confirmation.
              </p>
              <p style="font-family: var(--font-mono); font-size: var(--font-size-sm); color: var(--color-text-secondary); margin: 0;">
                Add <code>/public/demo-migration.mp4</code> or <code>/public/demo-migration-1.png</code> through
                <code>/public/demo-migration-3.png</code>.
              </p>
            </article>
          </div>
        </section>

        <!-- Problem Section - Asymmetric Layout -->
        <section id="problem" class="l-section">
          <div class="l-shell">
            <!-- Large hero card -->
            <div class="m-card" style="background: var(--gradient-brand-subtle); border: 1px solid var(--color-accent); margin-bottom: var(--space-2xl); padding: var(--space-3xl);">
              <div style="max-width: 65ch;">
                <p style="font-size: var(--font-size-sm); text-transform: uppercase; letter-spacing: 0.1em; color: var(--color-accent); margin-bottom: var(--space-md); font-weight: 700;">The Problem</p>
                <h2 style="font-size: var(--font-size-3xl); font-weight: 700; line-height: var(--line-height-tight); margin-bottom: var(--space-lg);">
                  Vendor CLIs are fragmented, incomplete, and inconsistent
                </h2>
                <p style="font-size: var(--font-size-lg); color: var(--color-text-secondary); line-height: var(--line-height-relaxed);">
                  Modern APIs are powerful and well-documented, but the developer experience is broken. You're forced to juggle a dozen different CLIs, each with partial coverage and different conventions.
                </p>
              </div>
            </div>

            <!-- Grid of problem cards -->
            <div style="display: grid; grid-template-columns: repeat(auto-fit, minmax(min(100%, 280px), 1fr)); gap: var(--space-lg); margin-bottom: var(--space-3xl);">
              <div class="m-card" style="background: var(--color-background-alt);">
                <div style="width: 2.5rem; height: 2.5rem; border-radius: var(--radius-md); background: rgba(191, 97, 106, 0.15); display: grid; place-items: center; margin-bottom: var(--space-md);">
                  <img src="/icon-problem-inconsistent.svg" alt="" style="width: 1.25rem; height: 1.25rem;" />
                </div>
                <h3 style="font-size: var(--font-size-lg); font-weight: 600; margin-bottom: var(--space-sm);">Inconsistent commands</h3>
                <p class="m-card__text">Nearly identical operations with completely different naming conventions across vendors.</p>
              </div>

              <div class="m-card" style="background: var(--color-background-alt);">
                <div style="width: 2.5rem; height: 2.5rem; border-radius: var(--radius-md); background: rgba(191, 97, 106, 0.15); display: grid; place-items: center; margin-bottom: var(--space-md);">
                  <img src="/icon-problem-coverage-gap.svg" alt="" style="width: 1.25rem; height: 1.25rem;" />
                </div>
                <h3 style="font-size: var(--font-size-lg); font-weight: 600; margin-bottom: var(--space-sm);">Partial coverage</h3>
                <p class="m-card__text">Incomplete API coverage forces you back to curl or writing custom scripts.</p>
              </div>

              <div class="m-card" style="background: var(--color-background-alt);">
                <div style="width: 2.5rem; height: 2.5rem; border-radius: var(--radius-md); background: rgba(191, 97, 106, 0.15); display: grid; place-items: center; margin-bottom: var(--space-md);">
                  <img src="/icon-plugin-fragmentation.svg" alt="" style="width: 1.25rem; height: 1.25rem;" />
                </div>
                <h3 style="font-size: var(--font-size-lg); font-weight: 600; margin-bottom: var(--space-sm);">Fragmented plugins</h3>
                <p class="m-card__text">Separate MCP servers for each vendor with even less functionality than the CLI.</p>
              </div>

              <div class="m-card" style="background: var(--color-background-alt);">
                <div style="width: 2.5rem; height: 2.5rem; border-radius: var(--radius-md); background: rgba(191, 97, 106, 0.15); display: grid; place-items: center; margin-bottom: var(--space-md);">
                  <img src="/icon-brittle-automation.svg" alt="" style="width: 1.25rem; height: 1.25rem;" />
                </div>
                <h3 style="font-size: var(--font-size-lg); font-weight: 600; margin-bottom: var(--space-sm);">Brittle automation</h3>
                <p class="m-card__text">Workflows living in opaque shell scripts that break with every vendor update.</p>
              </div>
            </div>

            <!-- Solution card -->
            <div class="m-card" style="background: linear-gradient(135deg, rgba(136, 192, 208, 0.1) 0%, rgba(129, 161, 193, 0.05) 100%); border: 1px solid var(--color-accent); padding: var(--space-3xl);">
              <div style="max-width: 65ch;">
                <p style="font-size: var(--font-size-sm); text-transform: uppercase; letter-spacing: 0.1em; color: var(--color-accent); margin-bottom: var(--space-md); font-weight: 700;">The Solution</p>
                <h2 style="font-size: var(--font-size-3xl); font-weight: 700; line-height: var(--line-height-tight); margin-bottom: var(--space-lg);">
                  One coherent operational surface
                </h2>
                <p style="font-size: var(--font-size-lg); color: var(--color-text-secondary); line-height: var(--line-height-relaxed); margin-bottom: var(--space-lg);">
                  Import an OpenAPI document. Oatty generates a consistent command surface reused by TUI, workflows, CLI, and MCP tooling.
                </p>
                <div style="display: flex; gap: var(--space-xl); flex-wrap: wrap; color: var(--color-accent); font-weight: 600;">
                  <span>✓ One interface</span>
                  <span>✓ One mental model</span>
                  <span>✓ One place to operate</span>
                </div>
              </div>
            </div>
          </div>
        </section>

        <!-- Core Principles - Magazine Layout -->
        <section id="principles" class="l-section">
          <div class="l-shell">
            <p style="font-size: var(--font-size-sm); text-transform: uppercase; letter-spacing: 0.1em; color: var(--color-accent); margin-bottom: var(--space-md); font-weight: 700;">Design Philosophy</p>
            <h2 style="font-size: var(--font-size-4xl); font-weight: 700; margin: 0 0 var(--space-2xl);">
              Structured execution, without terminal friction
            </h2>

            <!-- 2x2 Grid with varying sizes -->
            <div style="display: grid; grid-template-columns: repeat(auto-fit, minmax(min(100%, 320px), 1fr)); gap: var(--space-lg);">
              <!-- Large feature card - Discoverability -->
              <article class="m-card" style="grid-column: span 2; background: var(--gradient-brand-subtle); padding: var(--space-3xl); display: grid; grid-template-columns: 1fr 1fr; gap: var(--space-2xl); align-items: center;">
                <div>
                  <div style="width: 5rem; height: 5rem; border-radius: var(--radius-xl); background: rgba(136, 192, 208, 0.2); display: grid; place-items: center; padding: 1rem; margin-bottom: var(--space-lg);">
                    <img src="/icon-discoverability.svg" alt="" style="width: 100%; height: 100%;" />
                  </div>
                  <h3 style="font-size: var(--font-size-2xl); font-weight: 700; margin-bottom: var(--space-md);">Discoverability</h3>
                  <p style="font-size: var(--font-size-lg); color: var(--color-text-secondary); line-height: var(--line-height-relaxed);">
                    You should never need to memorize commands. Guided UI with inline search, contextual hints, and discoverable keybindings. If the API supports it, you can find it.
                  </p>
                </div>
                <div style="background: var(--color-background-alt); border-radius: var(--radius-lg); padding: var(--space-lg); font-family: var(--font-mono); font-size: var(--font-size-sm); color: var(--color-text-secondary);">
                  <div style="color: var(--color-accent); margin-bottom: var(--space-sm);">// Type to search</div>
                  <div>oatty<span style="color: var(--color-accent);">▊</span></div>
                  <div style="margin-top: var(--space-md); padding: var(--space-sm); background: rgba(136, 192, 208, 0.1); border-radius: var(--radius-sm);">
                    <div style="opacity: 0.7;">→ apps create</div>
                    <div style="opacity: 0.7;">→ apps list</div>
                    <div style="opacity: 0.7;">→ databases create</div>
                  </div>
                </div>
              </article>

              <!-- Compact cards -->
              <article class="m-card" style="background: var(--color-elevated);">
                <div style="width: 4rem; height: 4rem; border-radius: var(--radius-lg); background: var(--gradient-brand-subtle); display: grid; place-items: center; padding: 0.75rem; margin-bottom: var(--space-md);">
                  <img src="/icon-simplicity.svg" alt="" style="width: 100%; height: 100%;" />
                </div>
                <h3 style="font-size: var(--font-size-xl); font-weight: 700; margin-bottom: var(--space-sm);">Simplicity</h3>
                <p class="m-card__text">
                  Each screen does one thing, clearly. Search on top, results in center, details on right. No clutter, no overloaded views, no hidden modes.
                </p>
              </article>

              <article class="m-card" style="background: var(--color-elevated);">
                <div style="width: 4rem; height: 4rem; border-radius: var(--radius-lg); background: var(--gradient-brand-subtle); display: grid; place-items: center; padding: 0.75rem; margin-bottom: var(--space-md);">
                  <img src="/icon-speed.svg" alt="" style="width: 100%; height: 100%;" />
                </div>
                <h3 style="font-size: var(--font-size-xl); font-weight: 700; margin-bottom: var(--space-sm);">Speed</h3>
                <p class="m-card__text">
                  Designed for real work, not demos. Command prompt with <code style="font-family: var(--font-mono); background: var(--color-background-alt); padding: 0.125rem 0.375rem; border-radius: var(--radius-sm);">:</code> prefix, history navigation, instant autocomplete.
                </p>
              </article>

              <article class="m-card" style="background: var(--color-elevated);">
                <div style="width: 4rem; height: 4rem; border-radius: var(--radius-lg); background: var(--gradient-brand-subtle); display: grid; place-items: center; padding: 0.75rem; margin-bottom: var(--space-md);">
                  <img src="/icon-consistency.svg" alt="" style="width: 100%; height: 100%;" />
                </div>
                <h3 style="font-size: var(--font-size-xl); font-weight: 700; margin-bottom: var(--space-sm);">Consistency</h3>
                <p class="m-card__text">
                  Workflows behave like commands. The same search, execution, and logging interface across HTTP commands, MCP tools, and workflows.
                </p>
              </article>
            </div>
          </div>
        </section>

        <!-- Features - Staggered Layout -->
        <section id="features" class="l-section">
          <div class="l-shell">
            <p style="font-size: var(--font-size-sm); text-transform: uppercase; letter-spacing: 0.1em; color: var(--color-accent); margin-bottom: var(--space-md); font-weight: 700;">What You Get</p>
            <h2 style="font-size: var(--font-size-4xl); font-weight: 700; margin: 0 0 var(--space-3xl);">
              Core capabilities for daily operations
            </h2>

            <!-- Staggered feature cards -->
            <div style="display: flex; flex-direction: column; gap: var(--space-2xl);">

              <!-- Natural language assistance + safe execution -->
              <article class="m-card" style="background: var(--gradient-brand-subtle); padding: var(--space-3xl); border: 1px solid var(--color-accent); overflow: hidden;">
                <div style="margin-bottom: var(--space-2xl);">
                  <span style="font-size: var(--font-size-xs); text-transform: uppercase; letter-spacing: 0.1em; color: var(--color-accent); font-weight: 700;">Natural language assistance + safe execution</span>
                  <h3 style="font-size: var(--font-size-3xl); font-weight: 700; margin: var(--space-md) 0;">Describe the objective, then review before execution</h3>
                  <p style="font-size: var(--font-size-lg); color: var(--color-text-secondary); line-height: var(--line-height-relaxed); margin-bottom: var(--space-lg);">
                    Start in the TUI. Ask for what you want to accomplish, review suggested commands/workflow steps, validate inputs, and confirm before running.
                  </p>
                  <div style="display: flex; gap: var(--space-md); flex-wrap: wrap;">
                    <span style="padding: 0.375rem 0.75rem; background: var(--color-elevated); border-radius: var(--radius-full); font-size: var(--font-size-sm);">Suggest</span>
                    <span style="padding: 0.375rem 0.75rem; background: var(--color-elevated); border-radius: var(--radius-full); font-size: var(--font-size-sm);">Preview + Validate</span>
                    <span style="padding: 0.375rem 0.75rem; background: var(--color-elevated); border-radius: var(--radius-full); font-size: var(--font-size-sm);">Confirm + Run</span>
                  </div>
                </div>
                <img src="/Oatty-finder.png" alt="Oatty command finder with fuzzy search" class="m-screenshot" style="width: 100%; height: auto; border-radius: var(--radius-lg); border: 1px solid var(--color-divider); box-shadow: var(--shadow-xl);" @click="${()=>this.openLightbox("/Oatty-finder.png","Oatty command finder with fuzzy search")}" />
              </article>

              <!-- Three column row with screenshots - Core Features -->
              <div style="display: grid; grid-template-columns: repeat(auto-fit, minmax(min(100%, 320px), 1fr)); gap: var(--space-lg);">
                <article class="m-card" style="background: var(--color-elevated); overflow: hidden;">
                  <h3 style="font-size: var(--font-size-xl); font-weight: 700; margin-bottom: var(--space-sm); display: flex; align-items: center; gap: var(--space-sm);">
                    <img src="/icon-library.svg" alt="" style="width: 1.5rem; height: 1.5rem;" />
                    Library Management
                  </h3>
                  <p class="m-card__text" style="margin-bottom: var(--space-lg);">
                    Import OpenAPI specs through the TUI. Edit metadata, configure auth, enable/disable catalogs—all without leaving the terminal.
                  </p>
                  <img src="/Oatty-library.png" alt="Oatty library management interface" class="m-screenshot" style="width: 100%; height: auto; border-radius: var(--radius-md); border: 1px solid var(--color-divider); margin-top: auto;" @click="${()=>this.openLightbox("/Oatty-library.png","Oatty library management interface")}" />
                </article>

                <article class="m-card" style="background: var(--color-elevated); overflow: hidden;">
                  <h3 style="font-size: var(--font-size-xl); font-weight: 700; margin-bottom: var(--space-sm); display: flex; align-items: center; gap: var(--space-sm);">
                    <img src="/icon-workflow.svg" alt="" style="width: 1.5rem; height: 1.5rem;" />
                    Workflow Catalog
                  </h3>
                  <p class="m-card__text" style="margin-bottom: var(--space-lg);">
                    Browse, create, and manage workflows. Organize multi-step operations as reusable, shareable YAML definitions.
                  </p>
                  <img src="/Oatty-workflows-list.png" alt="Oatty workflow catalog" class="m-screenshot" style="width: 100%; height: auto; border-radius: var(--radius-md); border: 1px solid var(--color-divider); margin-top: auto;" @click="${()=>this.openLightbox("/Oatty-workflows-list.png","Oatty workflow catalog")}" />
                </article>

                <article class="m-card" style="background: var(--color-elevated); overflow: hidden;">
                  <h3 style="font-size: var(--font-size-xl); font-weight: 700; margin-bottom: var(--space-sm); display: flex; align-items: center; gap: var(--space-sm);">
                    <img src="/icon-run.svg" alt="" style="width: 1.5rem; height: 1.5rem;" />
                    Command Execution
                  </h3>
                  <p class="m-card__text" style="margin-bottom: var(--space-lg);">
                    Run workflows or direct API commands with rich output. See JSON responses, logs, and execution status in real-time.
                  </p>
                  <img src="/Oatty-run.png" alt="Oatty command execution output" class="m-screenshot" style="width: 100%; height: auto; border-radius: var(--radius-md); border: 1px solid var(--color-divider); margin-top: auto;" @click="${()=>this.openLightbox("/Oatty-run.png","Oatty command execution output")}" />
                </article>
              </div>

              <!-- Two column row with screenshots - MCP Integration -->
              <div style="display: grid; grid-template-columns: repeat(auto-fit, minmax(min(100%, 400px), 1fr)); gap: var(--space-lg);">
                <article class="m-card" style="background: var(--color-elevated); overflow: hidden;">
                  <h3 style="font-size: var(--font-size-xl); font-weight: 700; margin-bottom: var(--space-sm); display: flex; align-items: center; gap: var(--space-sm);">
                    <img src="/icon-mcp-server.svg" alt="" style="width: 1.5rem; height: 1.5rem;" />
                    MCP Server Mode
                  </h3>
                  <p class="m-card__text" style="margin-bottom: var(--space-lg);">
                    Run Oatty as an MCP server. All commands and workflows exposed as tools for AI assistants—Claude, Cline, or any MCP client.
                  </p>
                  <img src="/Oatty-mcp-server.png" alt="Oatty MCP server interface" class="m-screenshot" style="width: 100%; height: auto; border-radius: var(--radius-md); border: 1px solid var(--color-divider); margin-top: auto;" @click="${()=>this.openLightbox("/Oatty-mcp-server.png","Oatty MCP server interface")}" />
                </article>

                <article class="m-card" style="background: var(--color-elevated); overflow: hidden;">
                  <h3 style="font-size: var(--font-size-xl); font-weight: 700; margin-bottom: var(--space-sm); display: flex; align-items: center; gap: var(--space-sm);">
                    <img src="/icon-mcp-client.svg" alt="" style="width: 1.5rem; height: 1.5rem;" />
                    MCP Client Mode
                  </h3>
                  <p class="m-card__text" style="margin-bottom: var(--space-lg);">
                    Manage and execute tools from any MCP plugin. Native integrations with filesystem, GitHub, Postgres—all discoverable through the same TUI.
                  </p>
                  <img src="/Oatty-mcp-client.png" alt="Oatty MCP client interface" class="m-screenshot" style="width: 100%; height: auto; border-radius: var(--radius-md); border: 1px solid var(--color-divider); margin-top: auto;" @click="${()=>this.openLightbox("/Oatty-mcp-client.png","Oatty MCP client interface")}" />
                </article>
              </div>

              <!-- Three column row -->
              <div style="display: grid; grid-template-columns: repeat(auto-fit, minmax(min(100%, 240px), 1fr)); gap: var(--space-lg);">
                <article class="m-card" style="background: var(--color-surface);">
                  <h3 style="font-size: var(--font-size-lg); font-weight: 700; margin-bottom: var(--space-sm); display: flex; align-items: center; gap: var(--space-sm);">
                    <img src="/icon-value-providers.svg" alt="" style="width: 1rem; height: 1rem;" />
                    Value Providers
                  </h3>
                  <p class="m-card__text">
                    Intelligent autocomplete that fetches live values. Provider-backed suggestions with caching and dependency resolution.
                  </p>
                </article>

                <article class="m-card" style="background: var(--color-surface);">
                  <h3 style="font-size: var(--font-size-lg); font-weight: 700; margin-bottom: var(--space-sm); display: flex; align-items: center; gap: var(--space-sm);">
                    <img src="/icon-command-browser.svg" alt="" style="width: 1rem; height: 1rem;" />
                    Command Browser
                  </h3>
                  <p class="m-card__text">
                    Explore your entire API surface with searchable, categorized views. See methods, summaries, and details before execution.
                  </p>
                </article>

                <article class="m-card" style="background: var(--color-surface);">
                  <h3 style="font-size: var(--font-size-lg); font-weight: 700; margin-bottom: var(--space-sm); display: flex; align-items: center; gap: var(--space-sm);">
                    <img src="/icon-rich-logging.svg" alt="" style="width: 1rem; height: 1rem;" />
                    Rich Logging
                  </h3>
                  <p class="m-card__text">
                    Searchable logs panel with syntax highlighting. Toggle with Ctrl+L. See execution history and debug workflows.
                  </p>
                </article>
              </div>

              <!-- Full width feature -->
              <article class="m-card" style="background: var(--color-elevated); padding: var(--space-3xl);">
                <div style="max-width: 65ch;">
                  <h3 style="font-size: var(--font-size-2xl); font-weight: 700; margin-bottom: var(--space-md);">Schema-Driven Everything</h3>
                  <p style="font-size: var(--font-size-lg); color: var(--color-text-secondary); line-height: var(--line-height-relaxed); margin-bottom: var(--space-lg);">
                    Commands are derived from imported OpenAPI documents so your catalog stays aligned with the APIs you operate.
                  </p>
                  <div style="display: flex; gap: var(--space-xl); flex-wrap: wrap; color: var(--color-text-secondary); font-size: var(--font-size-sm);">
                    <div><strong style="color: var(--color-accent);">✓</strong> Consistent command generation from schema</div>
                    <div><strong style="color: var(--color-accent);">✓</strong> Re-import when API contracts change</div>
                    <div><strong style="color: var(--color-accent);">✓</strong> MCP tool integration</div>
                  </div>
                </div>
              </article>

            </div>
          </div>
        </section>

        <!-- Installation -->
        <section id="install" class="l-section" style="background: var(--gradient-brand-subtle); border-radius: var(--radius-xl); padding: var(--space-4xl) 0;">
          <div class="l-shell">
            <h2 style="font-size: var(--font-size-4xl); font-weight: 700; margin: 0 0 var(--space-xl); text-align: center;">
              Install and verify
            </h2>
            <div class="l-grid l-grid--two">
              <div class="m-card">
                <h3 class="m-card__title">Install via npm</h3>
                <pre class="m-code"><code>npm install -g oatty
oatty --help</code></pre>
                <p class="m-card__text">
                  Automatically downloads the matching prebuilt binary for your platform.
                </p>
              </div>
              <div class="m-card">
                <h3 class="m-card__title">Install from source</h3>
                <pre class="m-code"><code>cargo build --release
./target/release/oatty</code></pre>
                <p class="m-card__text">
                  Build from source with Rust 1.93+. Full control over compilation.
                </p>
              </div>
            </div>
            <div class="m-card" style="margin-top: var(--space-xl);">
              <h3 class="m-card__title">Quick Start</h3>
              <pre class="m-code"><code># Start interactive TUI
oatty

# Import an OpenAPI catalog
oatty import ./schemas/your-api.json --kind catalog

# Search for commands
oatty search "create order"

# Run a workflow
oatty workflow list
oatty workflow run deploy --input env=staging</code></pre>
              <p class="m-card__text">Use the guided flow in <a href="/docs/quick-start" @click="${this.navigate}">Quick Start docs</a>.</p>
            </div>
          </div>
        </section>

        <!-- Architecture -->
        <section class="l-section">
          <div class="l-shell">
            <h2 style="font-size: var(--font-size-4xl); font-weight: 700; margin: 0 0 var(--space-xl);">
              Architecture
            </h2>
            <div class="l-grid l-grid--three">
              <article class="m-card">
                <div style="color: var(--color-accent); font-size: var(--font-size-2xl); font-weight: 700; margin-bottom: var(--space-sm);">
                  01
                </div>
                <h3 class="m-card__title">Registry</h3>
                <p class="m-card__text">
                  Loads catalog manifests derived from OpenAPI documents. Stores configuration in <code style="font-family: var(--font-mono); font-size: var(--font-size-sm); color: var(--color-accent);">~/.config/oatty/</code>
                </p>
              </article>
              <article class="m-card">
                <div style="color: var(--color-accent); font-size: var(--font-size-2xl); font-weight: 700; margin-bottom: var(--space-sm);">
                  02
                </div>
                <h3 class="m-card__title">CLI / TUI</h3>
                <p class="m-card__text">
                  Builds command tree from registry. CLI routes to HTTP/MCP execution. TUI provides interactive search and composition.
                </p>
              </article>
              <article class="m-card">
                <div style="color: var(--color-accent); font-size: var(--font-size-2xl); font-weight: 700; margin-bottom: var(--space-sm);">
                  03
                </div>
                <h3 class="m-card__title">MCP Engine</h3>
                <p class="m-card__text">
                  Manages MCP plugin lifecycles. Injects tool commands into the registry at runtime for seamless integration.
                </p>
              </article>
            </div>
          </div>
        </section>

        <!-- Footer -->
        <footer class="l-footer">
          <div class="l-shell">
            <div style="display: flex; justify-content: space-between; align-items: center; flex-wrap: wrap; gap: var(--space-xl);">
              <div>
                <img src="/logo-lockup.svg" alt="Oatty - Schema-driven CLI+TUI+MCP" style="width: min(280px, 100%); height: auto; margin-bottom: var(--space-md); filter: drop-shadow(0 2px 4px rgba(0, 0, 0, 0.2));" />
              </div>
              <nav style="display: flex; gap: var(--space-xl); flex-wrap: wrap;">
                <a href="https://github.com/oattyio/oatty" target="_blank" rel="noopener" style="color: var(--color-text-secondary); text-decoration: none; font-size: var(--font-size-sm); transition: color var(--transition-fast);">
                  GitHub
                </a>
                <a href="https://github.com/oattyio/oatty#readme" target="_blank" rel="noopener" style="color: var(--color-text-secondary); text-decoration: none; font-size: var(--font-size-sm); transition: color var(--transition-fast);">
                  Documentation
                </a>
                <a href="https://github.com/oattyio/oatty/discussions" target="_blank" rel="noopener" style="color: var(--color-text-secondary); text-decoration: none; font-size: var(--font-size-sm); transition: color var(--transition-fast);">
                  Community
                </a>
                <a href="https://github.com/oattyio/oatty/issues" target="_blank" rel="noopener" style="color: var(--color-text-secondary); text-decoration: none; font-size: var(--font-size-sm); transition: color var(--transition-fast);">
                  Issues
                </a>
              </nav>
            </div>
            <div style="margin-top: var(--space-xl); padding-top: var(--space-xl); border-top: 1px solid var(--color-divider); text-align: center;">
              <p style="color: var(--color-text-muted); font-size: var(--font-size-sm); margin: 0;">
                MIT OR Apache-2.0 License • Built with Rust
              </p>
            </div>
          </div>
        </footer>
      </main>
    `}}customElements.define("oatty-site-app",rt);
