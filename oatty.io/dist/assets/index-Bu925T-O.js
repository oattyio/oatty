(function(){const e=document.createElement("link").relList;if(e&&e.supports&&e.supports("modulepreload"))return;for(const r of document.querySelectorAll('link[rel="modulepreload"]'))a(r);new MutationObserver(r=>{for(const i of r)if(i.type==="childList")for(const s of i.addedNodes)s.tagName==="LINK"&&s.rel==="modulepreload"&&a(s)}).observe(document,{childList:!0,subtree:!0});function t(r){const i={};return r.integrity&&(i.integrity=r.integrity),r.referrerPolicy&&(i.referrerPolicy=r.referrerPolicy),r.crossOrigin==="use-credentials"?i.credentials="include":r.crossOrigin==="anonymous"?i.credentials="omit":i.credentials="same-origin",i}function a(r){if(r.ep)return;r.ep=!0;const i=t(r);fetch(r.href,i)}})();/**
 * @license
 * Copyright 2019 Google LLC
 * SPDX-License-Identifier: BSD-3-Clause
 */const U=globalThis,B=U.ShadowRoot&&(U.ShadyCSS===void 0||U.ShadyCSS.nativeShadow)&&"adoptedStyleSheets"in Document.prototype&&"replace"in CSSStyleSheet.prototype,re=Symbol(),F=new WeakMap;let le=class{constructor(e,t,a){if(this._$cssResult$=!0,a!==re)throw Error("CSSResult is not constructable. Use `unsafeCSS` or `css` instead.");this.cssText=e,this.t=t}get styleSheet(){let e=this.o;const t=this.t;if(B&&e===void 0){const a=t!==void 0&&t.length===1;a&&(e=F.get(t)),e===void 0&&((this.o=e=new CSSStyleSheet).replaceSync(this.cssText),a&&F.set(t,e))}return e}toString(){return this.cssText}};const ce=o=>new le(typeof o=="string"?o:o+"",void 0,re),de=(o,e)=>{if(B)o.adoptedStyleSheets=e.map(t=>t instanceof CSSStyleSheet?t:t.styleSheet);else for(const t of e){const a=document.createElement("style"),r=U.litNonce;r!==void 0&&a.setAttribute("nonce",r),a.textContent=t.cssText,o.appendChild(a)}},W=B?o=>o:o=>o instanceof CSSStyleSheet?(e=>{let t="";for(const a of e.cssRules)t+=a.cssText;return ce(t)})(o):o;/**
 * @license
 * Copyright 2017 Google LLC
 * SPDX-License-Identifier: BSD-3-Clause
 */const{is:he,defineProperty:me,getOwnPropertyDescriptor:pe,getOwnPropertyNames:ge,getOwnPropertySymbols:ue,getPrototypeOf:fe}=Object,f=globalThis,V=f.trustedTypes,ve=V?V.emptyScript:"",N=f.reactiveElementPolyfillSupport,S=(o,e)=>o,D={toAttribute(o,e){switch(e){case Boolean:o=o?ve:null;break;case Object:case Array:o=o==null?o:JSON.stringify(o)}return o},fromAttribute(o,e){let t=o;switch(e){case Boolean:t=o!==null;break;case Number:t=o===null?null:Number(o);break;case Object:case Array:try{t=JSON.parse(o)}catch{t=null}}return t}},ae=(o,e)=>!he(o,e),J={attribute:!0,type:String,converter:D,reflect:!1,useDefault:!1,hasChanged:ae};Symbol.metadata??(Symbol.metadata=Symbol("metadata")),f.litPropertyMetadata??(f.litPropertyMetadata=new WeakMap);let _=class extends HTMLElement{static addInitializer(e){this._$Ei(),(this.l??(this.l=[])).push(e)}static get observedAttributes(){return this.finalize(),this._$Eh&&[...this._$Eh.keys()]}static createProperty(e,t=J){if(t.state&&(t.attribute=!1),this._$Ei(),this.prototype.hasOwnProperty(e)&&((t=Object.create(t)).wrapped=!0),this.elementProperties.set(e,t),!t.noAccessor){const a=Symbol(),r=this.getPropertyDescriptor(e,a,t);r!==void 0&&me(this.prototype,e,r)}}static getPropertyDescriptor(e,t,a){const{get:r,set:i}=pe(this.prototype,e)??{get(){return this[t]},set(s){this[t]=s}};return{get:r,set(s){const l=r==null?void 0:r.call(this);i==null||i.call(this,s),this.requestUpdate(e,l,a)},configurable:!0,enumerable:!0}}static getPropertyOptions(e){return this.elementProperties.get(e)??J}static _$Ei(){if(this.hasOwnProperty(S("elementProperties")))return;const e=fe(this);e.finalize(),e.l!==void 0&&(this.l=[...e.l]),this.elementProperties=new Map(e.elementProperties)}static finalize(){if(this.hasOwnProperty(S("finalized")))return;if(this.finalized=!0,this._$Ei(),this.hasOwnProperty(S("properties"))){const t=this.properties,a=[...ge(t),...ue(t)];for(const r of a)this.createProperty(r,t[r])}const e=this[Symbol.metadata];if(e!==null){const t=litPropertyMetadata.get(e);if(t!==void 0)for(const[a,r]of t)this.elementProperties.set(a,r)}this._$Eh=new Map;for(const[t,a]of this.elementProperties){const r=this._$Eu(t,a);r!==void 0&&this._$Eh.set(r,t)}this.elementStyles=this.finalizeStyles(this.styles)}static finalizeStyles(e){const t=[];if(Array.isArray(e)){const a=new Set(e.flat(1/0).reverse());for(const r of a)t.unshift(W(r))}else e!==void 0&&t.push(W(e));return t}static _$Eu(e,t){const a=t.attribute;return a===!1?void 0:typeof a=="string"?a:typeof e=="string"?e.toLowerCase():void 0}constructor(){super(),this._$Ep=void 0,this.isUpdatePending=!1,this.hasUpdated=!1,this._$Em=null,this._$Ev()}_$Ev(){var e;this._$ES=new Promise(t=>this.enableUpdating=t),this._$AL=new Map,this._$E_(),this.requestUpdate(),(e=this.constructor.l)==null||e.forEach(t=>t(this))}addController(e){var t;(this._$EO??(this._$EO=new Set)).add(e),this.renderRoot!==void 0&&this.isConnected&&((t=e.hostConnected)==null||t.call(e))}removeController(e){var t;(t=this._$EO)==null||t.delete(e)}_$E_(){const e=new Map,t=this.constructor.elementProperties;for(const a of t.keys())this.hasOwnProperty(a)&&(e.set(a,this[a]),delete this[a]);e.size>0&&(this._$Ep=e)}createRenderRoot(){const e=this.shadowRoot??this.attachShadow(this.constructor.shadowRootOptions);return de(e,this.constructor.elementStyles),e}connectedCallback(){var e;this.renderRoot??(this.renderRoot=this.createRenderRoot()),this.enableUpdating(!0),(e=this._$EO)==null||e.forEach(t=>{var a;return(a=t.hostConnected)==null?void 0:a.call(t)})}enableUpdating(e){}disconnectedCallback(){var e;(e=this._$EO)==null||e.forEach(t=>{var a;return(a=t.hostDisconnected)==null?void 0:a.call(t)})}attributeChangedCallback(e,t,a){this._$AK(e,a)}_$ET(e,t){var i;const a=this.constructor.elementProperties.get(e),r=this.constructor._$Eu(e,a);if(r!==void 0&&a.reflect===!0){const s=(((i=a.converter)==null?void 0:i.toAttribute)!==void 0?a.converter:D).toAttribute(t,a.type);this._$Em=e,s==null?this.removeAttribute(r):this.setAttribute(r,s),this._$Em=null}}_$AK(e,t){var i,s;const a=this.constructor,r=a._$Eh.get(e);if(r!==void 0&&this._$Em!==r){const l=a.getPropertyOptions(r),n=typeof l.converter=="function"?{fromAttribute:l.converter}:((i=l.converter)==null?void 0:i.fromAttribute)!==void 0?l.converter:D;this._$Em=r;const d=n.fromAttribute(t,l.type);this[r]=d??((s=this._$Ej)==null?void 0:s.get(r))??d,this._$Em=null}}requestUpdate(e,t,a,r=!1,i){var s;if(e!==void 0){const l=this.constructor;if(r===!1&&(i=this[e]),a??(a=l.getPropertyOptions(e)),!((a.hasChanged??ae)(i,t)||a.useDefault&&a.reflect&&i===((s=this._$Ej)==null?void 0:s.get(e))&&!this.hasAttribute(l._$Eu(e,a))))return;this.C(e,t,a)}this.isUpdatePending===!1&&(this._$ES=this._$EP())}C(e,t,{useDefault:a,reflect:r,wrapped:i},s){a&&!(this._$Ej??(this._$Ej=new Map)).has(e)&&(this._$Ej.set(e,s??t??this[e]),i!==!0||s!==void 0)||(this._$AL.has(e)||(this.hasUpdated||a||(t=void 0),this._$AL.set(e,t)),r===!0&&this._$Em!==e&&(this._$Eq??(this._$Eq=new Set)).add(e))}async _$EP(){this.isUpdatePending=!0;try{await this._$ES}catch(t){Promise.reject(t)}const e=this.scheduleUpdate();return e!=null&&await e,!this.isUpdatePending}scheduleUpdate(){return this.performUpdate()}performUpdate(){var a;if(!this.isUpdatePending)return;if(!this.hasUpdated){if(this.renderRoot??(this.renderRoot=this.createRenderRoot()),this._$Ep){for(const[i,s]of this._$Ep)this[i]=s;this._$Ep=void 0}const r=this.constructor.elementProperties;if(r.size>0)for(const[i,s]of r){const{wrapped:l}=s,n=this[i];l!==!0||this._$AL.has(i)||n===void 0||this.C(i,void 0,s,n)}}let e=!1;const t=this._$AL;try{e=this.shouldUpdate(t),e?(this.willUpdate(t),(a=this._$EO)==null||a.forEach(r=>{var i;return(i=r.hostUpdate)==null?void 0:i.call(r)}),this.update(t)):this._$EM()}catch(r){throw e=!1,this._$EM(),r}e&&this._$AE(t)}willUpdate(e){}_$AE(e){var t;(t=this._$EO)==null||t.forEach(a=>{var r;return(r=a.hostUpdated)==null?void 0:r.call(a)}),this.hasUpdated||(this.hasUpdated=!0,this.firstUpdated(e)),this.updated(e)}_$EM(){this._$AL=new Map,this.isUpdatePending=!1}get updateComplete(){return this.getUpdateComplete()}getUpdateComplete(){return this._$ES}shouldUpdate(e){return!0}update(e){this._$Eq&&(this._$Eq=this._$Eq.forEach(t=>this._$ET(t,this[t]))),this._$EM()}updated(e){}firstUpdated(e){}};_.elementStyles=[],_.shadowRootOptions={mode:"open"},_[S("elementProperties")]=new Map,_[S("finalized")]=new Map,N==null||N({ReactiveElement:_}),(f.reactiveElementVersions??(f.reactiveElementVersions=[])).push("2.1.2");/**
 * @license
 * Copyright 2017 Google LLC
 * SPDX-License-Identifier: BSD-3-Clause
 */const k=globalThis,G=o=>o,T=k.trustedTypes,Y=T?T.createPolicy("lit-html",{createHTML:o=>o}):void 0,oe="$lit$",u=`lit$${Math.random().toFixed(9).slice(2)}$`,ie="?"+u,ye=`<${ie}>`,b=document,C=()=>b.createComment(""),E=o=>o===null||typeof o!="object"&&typeof o!="function",q=Array.isArray,xe=o=>q(o)||typeof(o==null?void 0:o[Symbol.iterator])=="function",R=`[ 	
\f\r]`,A=/<(?:(!--|\/[^a-zA-Z])|(\/?[a-zA-Z][^>\s]*)|(\/?$))/g,K=/-->/g,Z=/>/g,v=RegExp(`>|${R}(?:([^\\s"'>=/]+)(${R}*=${R}*(?:[^ 	
\f\r"'\`<>=]|("|')|))|$)`,"g"),Q=/'/g,X=/"/g,se=/^(?:script|style|textarea|title)$/i,be=o=>(e,...t)=>({_$litType$:o,strings:e,values:t}),_e=be(1),w=Symbol.for("lit-noChange"),h=Symbol.for("lit-nothing"),ee=new WeakMap,y=b.createTreeWalker(b,129);function ne(o,e){if(!q(o)||!o.hasOwnProperty("raw"))throw Error("invalid template strings array");return Y!==void 0?Y.createHTML(e):e}const we=(o,e)=>{const t=o.length-1,a=[];let r,i=e===2?"<svg>":e===3?"<math>":"",s=A;for(let l=0;l<t;l++){const n=o[l];let d,m,c=-1,p=0;for(;p<n.length&&(s.lastIndex=p,m=s.exec(n),m!==null);)p=s.lastIndex,s===A?m[1]==="!--"?s=K:m[1]!==void 0?s=Z:m[2]!==void 0?(se.test(m[2])&&(r=RegExp("</"+m[2],"g")),s=v):m[3]!==void 0&&(s=v):s===v?m[0]===">"?(s=r??A,c=-1):m[1]===void 0?c=-2:(c=s.lastIndex-m[2].length,d=m[1],s=m[3]===void 0?v:m[3]==='"'?X:Q):s===X||s===Q?s=v:s===K||s===Z?s=A:(s=v,r=void 0);const g=s===v&&o[l+1].startsWith("/>")?" ":"";i+=s===A?n+ye:c>=0?(a.push(d),n.slice(0,c)+oe+n.slice(c)+u+g):n+u+(c===-2?l:g)}return[ne(o,i+(o[t]||"<?>")+(e===2?"</svg>":e===3?"</math>":"")),a]};class P{constructor({strings:e,_$litType$:t},a){let r;this.parts=[];let i=0,s=0;const l=e.length-1,n=this.parts,[d,m]=we(e,t);if(this.el=P.createElement(d,a),y.currentNode=this.el.content,t===2||t===3){const c=this.el.content.firstChild;c.replaceWith(...c.childNodes)}for(;(r=y.nextNode())!==null&&n.length<l;){if(r.nodeType===1){if(r.hasAttributes())for(const c of r.getAttributeNames())if(c.endsWith(oe)){const p=m[s++],g=r.getAttribute(c).split(u),M=/([.?@])?(.*)/.exec(p);n.push({type:1,index:i,name:M[2],strings:g,ctor:M[1]==="."?Ae:M[1]==="?"?Se:M[1]==="@"?ke:H}),r.removeAttribute(c)}else c.startsWith(u)&&(n.push({type:6,index:i}),r.removeAttribute(c));if(se.test(r.tagName)){const c=r.textContent.split(u),p=c.length-1;if(p>0){r.textContent=T?T.emptyScript:"";for(let g=0;g<p;g++)r.append(c[g],C()),y.nextNode(),n.push({type:2,index:++i});r.append(c[p],C())}}}else if(r.nodeType===8)if(r.data===ie)n.push({type:2,index:i});else{let c=-1;for(;(c=r.data.indexOf(u,c+1))!==-1;)n.push({type:7,index:i}),c+=u.length-1}i++}}static createElement(e,t){const a=b.createElement("template");return a.innerHTML=e,a}}function $(o,e,t=o,a){var s,l;if(e===w)return e;let r=a!==void 0?(s=t._$Co)==null?void 0:s[a]:t._$Cl;const i=E(e)?void 0:e._$litDirective$;return(r==null?void 0:r.constructor)!==i&&((l=r==null?void 0:r._$AO)==null||l.call(r,!1),i===void 0?r=void 0:(r=new i(o),r._$AT(o,t,a)),a!==void 0?(t._$Co??(t._$Co=[]))[a]=r:t._$Cl=r),r!==void 0&&(e=$(o,r._$AS(o,e.values),r,a)),e}class $e{constructor(e,t){this._$AV=[],this._$AN=void 0,this._$AD=e,this._$AM=t}get parentNode(){return this._$AM.parentNode}get _$AU(){return this._$AM._$AU}u(e){const{el:{content:t},parts:a}=this._$AD,r=((e==null?void 0:e.creationScope)??b).importNode(t,!0);y.currentNode=r;let i=y.nextNode(),s=0,l=0,n=a[0];for(;n!==void 0;){if(s===n.index){let d;n.type===2?d=new O(i,i.nextSibling,this,e):n.type===1?d=new n.ctor(i,n.name,n.strings,this,e):n.type===6&&(d=new ze(i,this,e)),this._$AV.push(d),n=a[++l]}s!==(n==null?void 0:n.index)&&(i=y.nextNode(),s++)}return y.currentNode=b,r}p(e){let t=0;for(const a of this._$AV)a!==void 0&&(a.strings!==void 0?(a._$AI(e,a,t),t+=a.strings.length-2):a._$AI(e[t])),t++}}class O{get _$AU(){var e;return((e=this._$AM)==null?void 0:e._$AU)??this._$Cv}constructor(e,t,a,r){this.type=2,this._$AH=h,this._$AN=void 0,this._$AA=e,this._$AB=t,this._$AM=a,this.options=r,this._$Cv=(r==null?void 0:r.isConnected)??!0}get parentNode(){let e=this._$AA.parentNode;const t=this._$AM;return t!==void 0&&(e==null?void 0:e.nodeType)===11&&(e=t.parentNode),e}get startNode(){return this._$AA}get endNode(){return this._$AB}_$AI(e,t=this){e=$(this,e,t),E(e)?e===h||e==null||e===""?(this._$AH!==h&&this._$AR(),this._$AH=h):e!==this._$AH&&e!==w&&this._(e):e._$litType$!==void 0?this.$(e):e.nodeType!==void 0?this.T(e):xe(e)?this.k(e):this._(e)}O(e){return this._$AA.parentNode.insertBefore(e,this._$AB)}T(e){this._$AH!==e&&(this._$AR(),this._$AH=this.O(e))}_(e){this._$AH!==h&&E(this._$AH)?this._$AA.nextSibling.data=e:this.T(b.createTextNode(e)),this._$AH=e}$(e){var i;const{values:t,_$litType$:a}=e,r=typeof a=="number"?this._$AC(e):(a.el===void 0&&(a.el=P.createElement(ne(a.h,a.h[0]),this.options)),a);if(((i=this._$AH)==null?void 0:i._$AD)===r)this._$AH.p(t);else{const s=new $e(r,this),l=s.u(this.options);s.p(t),this.T(l),this._$AH=s}}_$AC(e){let t=ee.get(e.strings);return t===void 0&&ee.set(e.strings,t=new P(e)),t}k(e){q(this._$AH)||(this._$AH=[],this._$AR());const t=this._$AH;let a,r=0;for(const i of e)r===t.length?t.push(a=new O(this.O(C()),this.O(C()),this,this.options)):a=t[r],a._$AI(i),r++;r<t.length&&(this._$AR(a&&a._$AB.nextSibling,r),t.length=r)}_$AR(e=this._$AA.nextSibling,t){var a;for((a=this._$AP)==null?void 0:a.call(this,!1,!0,t);e!==this._$AB;){const r=G(e).nextSibling;G(e).remove(),e=r}}setConnected(e){var t;this._$AM===void 0&&(this._$Cv=e,(t=this._$AP)==null||t.call(this,e))}}class H{get tagName(){return this.element.tagName}get _$AU(){return this._$AM._$AU}constructor(e,t,a,r,i){this.type=1,this._$AH=h,this._$AN=void 0,this.element=e,this.name=t,this._$AM=r,this.options=i,a.length>2||a[0]!==""||a[1]!==""?(this._$AH=Array(a.length-1).fill(new String),this.strings=a):this._$AH=h}_$AI(e,t=this,a,r){const i=this.strings;let s=!1;if(i===void 0)e=$(this,e,t,0),s=!E(e)||e!==this._$AH&&e!==w,s&&(this._$AH=e);else{const l=e;let n,d;for(e=i[0],n=0;n<i.length-1;n++)d=$(this,l[a+n],t,n),d===w&&(d=this._$AH[n]),s||(s=!E(d)||d!==this._$AH[n]),d===h?e=h:e!==h&&(e+=(d??"")+i[n+1]),this._$AH[n]=d}s&&!r&&this.j(e)}j(e){e===h?this.element.removeAttribute(this.name):this.element.setAttribute(this.name,e??"")}}class Ae extends H{constructor(){super(...arguments),this.type=3}j(e){this.element[this.name]=e===h?void 0:e}}class Se extends H{constructor(){super(...arguments),this.type=4}j(e){this.element.toggleAttribute(this.name,!!e&&e!==h)}}class ke extends H{constructor(e,t,a,r,i){super(e,t,a,r,i),this.type=5}_$AI(e,t=this){if((e=$(this,e,t,0)??h)===w)return;const a=this._$AH,r=e===h&&a!==h||e.capture!==a.capture||e.once!==a.once||e.passive!==a.passive,i=e!==h&&(a===h||r);r&&this.element.removeEventListener(this.name,this,a),i&&this.element.addEventListener(this.name,this,e),this._$AH=e}handleEvent(e){var t;typeof this._$AH=="function"?this._$AH.call(((t=this.options)==null?void 0:t.host)??this.element,e):this._$AH.handleEvent(e)}}class ze{constructor(e,t,a){this.element=e,this.type=6,this._$AN=void 0,this._$AM=t,this.options=a}get _$AU(){return this._$AM._$AU}_$AI(e){$(this,e)}}const L=k.litHtmlPolyfillSupport;L==null||L(P,O),(k.litHtmlVersions??(k.litHtmlVersions=[])).push("3.3.2");const Ce=(o,e,t)=>{const a=(t==null?void 0:t.renderBefore)??e;let r=a._$litPart$;if(r===void 0){const i=(t==null?void 0:t.renderBefore)??null;a._$litPart$=r=new O(e.insertBefore(C(),i),i,void 0,t??{})}return r._$AI(o),r};/**
 * @license
 * Copyright 2017 Google LLC
 * SPDX-License-Identifier: BSD-3-Clause
 */const x=globalThis;class z extends _{constructor(){super(...arguments),this.renderOptions={host:this},this._$Do=void 0}createRenderRoot(){var t;const e=super.createRenderRoot();return(t=this.renderOptions).renderBefore??(t.renderBefore=e.firstChild),e}update(e){const t=this.render();this.hasUpdated||(this.renderOptions.isConnected=this.isConnected),super.update(e),this._$Do=Ce(t,this.renderRoot,this.renderOptions)}connectedCallback(){var e;super.connectedCallback(),(e=this._$Do)==null||e.setConnected(!0)}disconnectedCallback(){var e;super.disconnectedCallback(),(e=this._$Do)==null||e.setConnected(!1)}render(){return w}}var te;z._$litElement$=!0,z.finalized=!0,(te=x.litElementHydrateSupport)==null||te.call(x,{LitElement:z});const j=x.litElementPolyfillSupport;j==null||j({LitElement:z});(x.litElementVersions??(x.litElementVersions=[])).push("4.2.2");function Ee(){return"adoptedStyleSheets"in Document.prototype&&"replaceSync"in CSSStyleSheet.prototype}function I(o){if(!Ee())return{sheet:null,text:o};const e=new CSSStyleSheet;return e.replaceSync(o),{sheet:e,text:o}}function Pe(o,e){const t=e.map(i=>i.sheet).filter(i=>i!==null);if(t.length===e.length){o.adoptedStyleSheets=t;return}const a="oatty-fallback-styles";let r=o.querySelector(`style[data-style-id='${a}']`);r||(r=document.createElement("style"),r.dataset.styleId=a,o.prepend(r)),r.textContent=e.map(i=>i.text).join(`
`)}const Oe=":host{display:block;min-height:100vh;font-family:var(--font-sans);background:var(--surface-glow);color:var(--color-text-primary);line-height:var(--line-height-normal)}*{box-sizing:border-box;margin:0;padding:0}:where(h1,h2,h3,h4,h5,h6){margin:0;font-weight:700;line-height:var(--line-height-tight);color:var(--color-text-primary)}:where(p){margin:0;line-height:var(--line-height-relaxed)}:where(a){color:var(--color-accent);text-decoration:none;transition:color var(--transition-fast)}:where(a:hover){color:var(--color-accent-strong)}:where(button,select){font:inherit;color:inherit;background:none;border:none;cursor:pointer}:where(button,a,select):focus-visible{outline:2px solid var(--color-focus);outline-offset:2px;border-radius:var(--radius-sm)}:where(ul,ol){list-style-position:inside}:where(code){font-family:var(--font-mono);font-size:.9em}@media(prefers-reduced-motion:reduce){*,*:before,*:after{animation-duration:1ms!important;animation-iteration-count:1!important;scroll-behavior:auto!important;transition-duration:1ms!important}}",Ie=I(Oe),Me=".l-container{width:min(var(--page-max-width),calc(100% - var(--space-xl)));margin-inline:auto;padding-inline:var(--space-md)}.l-shell{width:min(var(--page-max-width),calc(100% - var(--space-xl)));margin-inline:auto}.l-header{position:sticky;top:0;z-index:50;-webkit-backdrop-filter:blur(12px) saturate(180%);backdrop-filter:blur(12px) saturate(180%);background:#1a1f2ed9;border-bottom:1px solid var(--color-divider);height:var(--nav-height)}.l-header .l-header__inner{display:flex;align-items:center;justify-content:space-between;gap:var(--space-lg);height:100%}.l-main{min-height:calc(100vh - var(--nav-height));padding:var(--space-4xl) 0}.l-hero{display:grid;gap:var(--space-3xl);padding:var(--space-4xl) 0;background:var(--hero-glow)}.l-hero__content{max-width:65ch;margin:0 auto;text-align:center}.l-section{padding:var(--space-4xl) 0}.l-section+.l-section{border-top:1px solid var(--color-divider)}.l-grid{display:grid;gap:var(--space-lg)}.l-grid--two{grid-template-columns:repeat(auto-fit,minmax(min(100%,320px),1fr))}.l-grid--three{grid-template-columns:repeat(auto-fit,minmax(min(100%,280px),1fr))}.l-grid--four{grid-template-columns:repeat(auto-fit,minmax(min(100%,240px),1fr))}.l-grid--features{grid-template-columns:repeat(auto-fit,minmax(min(100%,320px),1fr));gap:var(--space-xl)}.l-split{display:grid;grid-template-columns:minmax(0,1fr) 280px;gap:var(--space-3xl);align-items:start}.l-docs-layout{display:grid;grid-template-columns:240px minmax(0,1fr) 240px;gap:var(--space-3xl);align-items:start}.l-docs-layout__content{display:flex;flex-direction:column;gap:var(--space-3xl);max-width:var(--content-max-width)}.l-footer{border-top:1px solid var(--color-divider);padding:var(--space-3xl) 0;margin-top:var(--space-4xl)}.l-stack{display:flex;flex-direction:column;gap:var(--space-md)}.l-stack--sm{gap:var(--space-sm)}.l-stack--lg{gap:var(--space-lg)}.l-stack--xl{gap:var(--space-xl)}.l-flex{display:flex;gap:var(--space-md)}.l-flex--center{align-items:center;justify-content:center}.l-flex--between{align-items:center;justify-content:space-between}.l-flex--wrap{flex-wrap:wrap}@media(max-width:1024px){.l-docs-layout{grid-template-columns:minmax(0,1fr) 220px}.l-split{grid-template-columns:minmax(0,1fr)}}@media(max-width:768px){.l-container,.l-shell{width:calc(100% - var(--space-lg))}.l-main,.l-hero,.l-section{padding:var(--space-2xl) 0}.l-docs-layout{grid-template-columns:minmax(0,1fr)}.l-grid--features{gap:var(--space-lg)}}@media(max-width:640px){.l-header .l-header__inner{gap:var(--space-md)}.l-hero,.l-section{padding:var(--space-xl) 0}}",Ue=I(Me),Te='.m-logo{display:inline-flex;align-items:center;gap:var(--space-sm);font-weight:700;letter-spacing:.04em;text-decoration:none;color:var(--color-text-primary);transition:transform var(--transition-fast),opacity var(--transition-fast)}.m-logo:hover{opacity:.9;transform:translateY(-1px)}.m-logo img{display:block;width:2rem;height:2rem;filter:drop-shadow(0 2px 4px rgba(0,0,0,.3))}.m-logo .m-logo__glyph{display:grid;place-items:center;width:2rem;height:2rem;border-radius:var(--radius-sm);border:1px solid var(--color-accent-subtle);background:var(--color-accent-subtle);font-family:var(--font-mono);font-size:var(--font-size-sm);color:var(--color-accent)}.m-logo .m-logo__tag{font-size:var(--font-size-xs);color:var(--color-text-tertiary);border:1px solid var(--color-divider);border-radius:var(--radius-full);padding:.1rem .45rem}.m-skip-link{position:absolute;left:-9999px;top:var(--space-sm);z-index:100;border-radius:var(--radius-md);background:var(--color-elevated);border:1px solid var(--color-surface-border);padding:var(--space-sm) var(--space-md);color:var(--color-text-primary);text-decoration:none}.m-skip-link:focus{left:var(--space-sm)}.m-nav{display:inline-flex;align-items:center;gap:var(--space-lg)}.m-nav .m-nav__link{color:var(--color-text-secondary);font-size:var(--font-size-sm);text-decoration:none;transition:color var(--transition-fast);position:relative}.m-nav .m-nav__link:hover{color:var(--color-text-primary)}.m-nav .m-nav__link:hover:after{content:"";position:absolute;bottom:-.25rem;left:0;right:0;height:2px;background:var(--gradient-brand);border-radius:var(--radius-full)}.m-header-actions{display:inline-flex;align-items:center;gap:var(--space-md)}.m-search{display:inline-flex;align-items:center;gap:var(--space-sm);border:1px solid var(--color-divider);border-radius:var(--radius-md);padding:.5rem var(--space-md);color:var(--color-text-secondary);background:var(--color-surface);font-size:var(--font-size-sm);text-decoration:none;transition:all var(--transition-fast)}.m-search:hover{border-color:var(--color-surface-border);background:var(--color-elevated)}.m-search kbd{border:1px solid var(--color-divider);border-radius:var(--radius-sm);padding:.08rem .32rem;font-family:var(--font-mono);font-size:var(--font-size-xs);color:var(--color-text-primary);background:var(--color-background-alt)}.m-section{display:grid;gap:1rem;padding-top:.3rem}.m-section+.m-section{border-top:1px solid rgb(76 86 106 / .32);padding-top:1.9rem}.m-section__title{font-size:clamp(1.35rem,2vw,1.95rem);letter-spacing:.01em}.m-hero{display:grid;grid-template-columns:1.12fr .88fr;gap:1.1rem;align-items:stretch}.m-hero .m-hero__content{display:grid;gap:1rem;border:1px solid var(--color-surface-border);border-radius:.8rem;background:#3b425273;padding:clamp(1rem,2vw,1.5rem)}.m-hero .m-hero__headline{font-size:clamp(2rem,5.2vw,3.2rem);line-height:1.02;max-width:20ch;text-wrap:balance}.m-hero .m-hero__summary{max-width:65ch;color:var(--color-text-secondary);font-size:clamp(1.02rem,2.1vw,1.15rem);line-height:1.55}.m-hero .m-hero__subtext{color:var(--color-text-secondary);font-size:.92rem}.m-hero .m-hero__actions{display:flex;flex-wrap:wrap;gap:.7rem}.m-terminal-demo{border:1px solid var(--color-surface-border);border-radius:.8rem;background:linear-gradient(180deg,#242933f2,#242933bf);padding:1rem;display:grid;gap:.8rem}.m-terminal-demo .m-terminal-demo__title{color:var(--color-text-secondary);font-size:.76rem;text-transform:uppercase;letter-spacing:.08em}.m-terminal-demo .m-terminal-demo__window{border-radius:.65rem;border:1px solid rgb(76 86 106 / .8);background:#1f242d;padding:.9rem;min-height:260px;display:grid;gap:.8rem;align-content:start;overflow:hidden}.m-terminal-demo .m-terminal-demo__line{margin:0;font-family:Iosevka Term,JetBrains Mono,ui-monospace,monospace;color:#d8dee9;font-size:.82rem;white-space:nowrap;overflow:hidden;border-right:2px solid #88c0d0;animation:type-line 4.8s steps(70,end) infinite}.m-terminal-demo .m-terminal-demo__line--accent{color:#a3be8c;animation-delay:1.2s}@keyframes type-line{0%{max-width:0;opacity:0}10%{opacity:1}70%{max-width:100%;opacity:1}90%{max-width:100%;opacity:.9}to{max-width:0;opacity:0}}.m-button{display:inline-flex;align-items:center;justify-content:center;gap:var(--space-sm);border:1px solid var(--color-surface-border);border-radius:var(--radius-md);padding:.625rem var(--space-lg);font-weight:600;font-size:var(--font-size-sm);text-decoration:none;cursor:pointer;transition:all var(--transition-fast);background:var(--color-surface);color:var(--color-text-primary)}.m-button:hover{transform:translateY(-1px);border-color:var(--color-accent);box-shadow:var(--shadow-md)}.m-button:active{transform:translateY(0)}.m-button--primary{background:var(--gradient-brand);color:var(--color-background);border-color:transparent;font-weight:700}.m-button--primary:hover{box-shadow:var(--shadow-glow);opacity:.95}.m-card{border:1px solid var(--color-divider);background:var(--color-surface);border-radius:var(--radius-lg);padding:var(--space-xl);display:flex;flex-direction:column;gap:var(--space-md);transition:all var(--transition-base)}.m-card:hover{border-color:var(--color-surface-border);box-shadow:var(--shadow-md)}.m-card .m-card__title{font-size:var(--font-size-lg);font-weight:600;color:var(--color-text-primary)}.m-card .m-card__text{color:var(--color-text-secondary);line-height:var(--line-height-relaxed);font-size:var(--font-size-base)}.m-card--visual{align-content:start}.m-list{margin:0;padding-left:1.2rem;display:grid;gap:.6rem;color:var(--color-text-secondary);line-height:1.5}.m-visual__caption{margin:0;color:var(--color-text-secondary);font-size:.77rem;text-transform:uppercase;letter-spacing:.08em}.m-cli-pile{display:grid;gap:.5rem}.m-cli-pile span{border:1px dashed var(--color-surface-border);border-radius:.5rem;padding:.45rem .55rem;color:var(--color-text-secondary);font-family:Iosevka Term,JetBrains Mono,ui-monospace,monospace;font-size:.82rem}.m-visual__arrow{text-align:center;color:var(--color-accent);font-weight:700;margin:.2rem 0}.m-one-tool{border:1px solid rgb(136 192 208 / .72);background:#88c0d021;border-radius:.6rem;padding:.65rem .7rem;text-align:center;font-weight:700}.m-principle{align-content:start}.m-principle__icon{width:1.65rem;height:1.65rem;border:1px solid var(--color-surface-border);border-radius:.35rem;display:grid;place-items:center;color:var(--color-accent);font-weight:700;background:#88c0d014;font-size:.84rem}.m-micro-example{margin:0;color:var(--color-text-secondary);font-size:.82rem}.m-micro-example code{color:var(--color-text-primary);font-family:Iosevka Term,JetBrains Mono,ui-monospace,monospace}.m-section--callout{padding:clamp(.8rem,2vw,1.2rem);border:1px solid rgb(136 192 208 / .4);border-radius:.72rem;background:#81a1c114}.m-video-placeholder{min-height:240px;border-radius:.8rem;border:1px dashed rgb(136 192 208 / .9);background:linear-gradient(180deg,#242933e6,#1f242df2);display:grid;place-items:center;color:#88c0d0;font-weight:700;letter-spacing:.03em}.m-feature{align-content:start}.m-media-placeholder{min-height:140px;border-radius:.64rem;border:1px dashed var(--color-surface-border);background:#242933e6;display:grid;place-items:center;color:var(--color-text-secondary);font-size:.78rem;text-transform:uppercase;letter-spacing:.07em}.m-steps{position:relative}.m-step{align-content:center;min-height:126px}.m-step .m-step__number{margin:0;color:var(--color-accent);font-weight:700;font-family:Iosevka Term,JetBrains Mono,ui-monospace,monospace;font-size:.87rem}.m-quote{gap:.75rem}.m-code{margin:0;overflow:auto;border-radius:var(--radius-md);border:1px solid var(--color-divider);background:var(--color-background-alt);padding:var(--space-lg);font-family:var(--font-mono);font-size:var(--font-size-sm);line-height:var(--line-height-relaxed);color:var(--color-text-secondary);box-shadow:var(--shadow-sm)}.m-code code{font-family:inherit;color:var(--color-text-primary)}.m-code--hero{margin-top:var(--space-md);background:var(--color-background)}.m-badges{display:flex;flex-wrap:wrap;gap:.8rem}.m-badges img{max-width:100%;height:auto}.m-toc{position:sticky;top:5rem;display:grid;gap:.5rem;border-left:1px solid rgb(76 86 106 / .5);padding-left:.85rem}.m-toc .m-toc__title{margin:0 0 .2rem;color:var(--color-text-secondary);text-transform:uppercase;font-size:.72rem;letter-spacing:.09em}.m-toc a{color:var(--color-text-secondary);font-size:.82rem}.m-toc a:hover{color:var(--color-text-primary)}.m-footer__links{display:flex;flex-wrap:wrap;gap:1rem;color:var(--color-text-secondary);font-size:.9rem}@media(max-width:1100px){.m-toc{display:none}}@media(max-width:980px){.m-hero{grid-template-columns:minmax(0,1fr)}}@media(max-width:740px){.m-nav{width:100%;justify-content:flex-start;flex-wrap:wrap}.m-header-actions{width:100%;justify-content:flex-start}}',He=I(Te),Ne=".is-muted{color:var(--color-text-secondary)}.is-hidden{display:none!important}",Re=I(Ne),Le=':host,:root,:root[data-theme=system],:root[data-theme=light],:root[data-theme=dark],:root[data-theme=high-contrast]{color-scheme:dark;--color-background: #1a1f2e;--color-background-alt: #151922;--color-surface: #242933;--color-elevated: #2e3440;--color-surface-border: #3b4252;--color-divider: rgba(236, 239, 244, .08);--color-text-primary: #eceff4;--color-text-secondary: #d8dee9;--color-text-tertiary: #88929f;--color-text-muted: #616e88;--color-accent: #88c0d0;--color-accent-strong: #81a1c1;--color-accent-hover: #5e81ac;--color-accent-subtle: rgba(136, 192, 208, .12);--color-focus: #ebcb8b;--color-good: #a3be8c;--color-warning: #ebcb8b;--color-bad: #bf616a;--color-info: #88c0d0;--gradient-brand: linear-gradient(135deg, #88c0d0 0%, #81a1c1 50%, #5e81ac 100%);--gradient-brand-subtle: linear-gradient(135deg, rgba(136, 192, 208, .2) 0%, rgba(129, 161, 193, .15) 100%);--surface-glow: radial-gradient(1200px circle at 15% -5%, rgba(136, 192, 208, .15), transparent 55%), radial-gradient(900px circle at 85% 10%, rgba(129, 161, 193, .12), transparent 50%), linear-gradient(180deg, #1a1f2e 0%, #12161f 100%);--hero-glow: radial-gradient(800px circle at 50% -20%, rgba(136, 192, 208, .2), transparent 60%), radial-gradient(600px circle at 80% 30%, rgba(129, 161, 193, .15), transparent 55%);--shadow-sm: 0 1px 2px 0 rgba(0, 0, 0, .3);--shadow-md: 0 4px 6px -1px rgba(0, 0, 0, .4), 0 2px 4px -1px rgba(0, 0, 0, .3);--shadow-lg: 0 10px 15px -3px rgba(0, 0, 0, .5), 0 4px 6px -2px rgba(0, 0, 0, .4);--shadow-xl: 0 20px 25px -5px rgba(0, 0, 0, .5), 0 10px 10px -5px rgba(0, 0, 0, .4);--shadow-glow: 0 0 20px rgba(136, 192, 208, .3);--space-xs: .25rem;--space-sm: .5rem;--space-md: 1rem;--space-lg: 1.5rem;--space-xl: 2rem;--space-2xl: 3rem;--space-3xl: 4rem;--space-4xl: 6rem;--radius-sm: .25rem;--radius-md: .5rem;--radius-lg: .75rem;--radius-xl: 1rem;--radius-full: 9999px;--font-sans: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, "Helvetica Neue", Arial, sans-serif;--font-mono: "SF Mono", Monaco, "Cascadia Code", "Roboto Mono", Consolas, "Courier New", monospace;--font-size-xs: .75rem;--font-size-sm: .875rem;--font-size-base: 1rem;--font-size-lg: 1.125rem;--font-size-xl: 1.25rem;--font-size-2xl: 1.5rem;--font-size-3xl: 1.875rem;--font-size-4xl: 2.25rem;--font-size-5xl: 3rem;--line-height-tight: 1.25;--line-height-normal: 1.5;--line-height-relaxed: 1.75;--transition-fast: .15s cubic-bezier(.4, 0, .2, 1);--transition-base: .25s cubic-bezier(.4, 0, .2, 1);--transition-slow: .35s cubic-bezier(.4, 0, .2, 1);--content-max-width: 80ch;--page-max-width: 1280px;--nav-height: 4rem}',je=I(Le);class De extends z{createRenderRoot(){return this.attachShadow({mode:"open"})}connectedCallback(){super.connectedCallback(),this.shadowRoot&&Pe(this.shadowRoot,[je,Ie,Ue,He,Re])}render(){return _e`
      <a class="m-skip-link" href="#main-content">Skip to content</a>

      <header class="l-header">
        <div class="l-shell l-header__inner">
          <a href="#" class="m-logo" aria-label="Oatty home">
            <img src="/logo-icon.svg" alt="Oatty logo" style="width: 2rem; height: 2rem;" />
            <span style="font-size: var(--font-size-lg); font-weight: 700; letter-spacing: 0.05em;">OATTY</span>
          </a>
          <nav class="m-nav" aria-label="Primary">
            <a class="m-nav__link" href="#problem">Problem</a>
            <a class="m-nav__link" href="#principles">Principles</a>
            <a class="m-nav__link" href="#features">Features</a>
            <a class="m-nav__link" href="#install">Install</a>
          </nav>
          <div class="m-header-actions">
            <a class="m-button m-button--primary" href="https://github.com/oattyio/oatty" target="_blank" rel="noopener">
              Get Started
            </a>
          </div>
        </div>
      </header>

      <main id="main-content">
        <!-- Hero Section -->
        <section class="l-hero">
          <div class="l-shell">
            <div class="l-hero__content">
              <h1 style="font-size: var(--font-size-5xl); font-weight: 700; line-height: var(--line-height-tight); margin: 0; text-wrap: balance;">
                One CLI for Every API
              </h1>
              <p style="font-size: var(--font-size-xl); color: var(--color-text-secondary); line-height: var(--line-height-relaxed); margin: var(--space-lg) 0;">
                Schema-driven command discovery, interactive terminal UI, and extension via MCP. Stop juggling vendor CLIs.
              </p>
              <div class="l-flex" style="justify-content: center; margin-top: var(--space-xl);">
                <a class="m-button m-button--primary" href="#install">Get Started</a>
                <a class="m-button" href="https://github.com/oattyio/oatty" target="_blank" rel="noopener">View on GitHub</a>
              </div>
              <pre class="m-code m-code--hero" style="max-width: 600px; margin: var(--space-2xl) auto 0;"><code>npm install -g oatty

oatty catalog import ./api-schema.json
oatty search "create customer"
oatty workflow run deploy --input env=prod</code></pre>
            </div>
          </div>
        </section>

        <!-- Problem Section -->
        <section id="problem" class="l-section">
          <div class="l-shell">
            <h2 style="font-size: var(--font-size-4xl); font-weight: 700; margin: 0 0 var(--space-xl);">
              Why Oatty Exists
            </h2>
            <div class="l-grid l-grid--two">
              <div class="m-card">
                <h3 class="m-card__title">The Problem</h3>
                <p class="m-card__text">
                  Modern developer tooling has a paradox: APIs are powerful and well-documented, but the tools built on them are fragmented, incomplete, and inconsistent.
                </p>
                <ul style="color: var(--color-text-secondary); line-height: var(--line-height-relaxed); margin: 0; padding-left: var(--space-xl);">
                  <li>Nearly identical commands with different naming conventions</li>
                  <li>Partial API coverage forcing you back to curl</li>
                  <li>Separate MCP servers with even less functionality</li>
                  <li>Automation living in brittle scripts</li>
                </ul>
              </div>
              <div class="m-card">
                <h3 class="m-card__title">The Solution</h3>
                <p class="m-card__text">
                  Oatty collapses this complexity into <strong>one coherent operational surface</strong>.
                  It turns OpenAPI documents into runnable commands, provides a consistent CLI/TUI, and extends with MCP tools.
                </p>
                <p class="m-card__text" style="color: var(--color-accent);">
                  One interface. One mental model. One place to operate.
                </p>
              </div>
            </div>
          </div>
        </section>

        <!-- Core Principles -->
        <section id="principles" class="l-section">
          <div class="l-shell">
            <h2 style="font-size: var(--font-size-4xl); font-weight: 700; margin: 0 0 var(--space-xl);">
              Core UX Principles
            </h2>
            <div class="l-grid l-grid--four">
              <article class="m-card">
                <div style="width: 4rem; height: 4rem; border-radius: var(--radius-lg); background: var(--gradient-brand-subtle); display: grid; place-items: center; padding: 0.75rem;">
                  <img src="/icon-discoverability.svg" alt="" style="width: 100%; height: 100%;" />
                </div>
                <h3 class="m-card__title">Discoverability</h3>
                <p class="m-card__text">
                  Never memorize commands. Every command and workflow is searchable and browsable. If the API supports it, you can find it.
                </p>
              </article>
              <article class="m-card">
                <div style="width: 4rem; height: 4rem; border-radius: var(--radius-lg); background: var(--gradient-brand-subtle); display: grid; place-items: center; padding: 0.75rem;">
                  <img src="/icon-simplicity.svg" alt="" style="width: 100%; height: 100%;" />
                </div>
                <h3 class="m-card__title">Simplicity</h3>
                <p class="m-card__text">
                  Each screen does one thing clearly. No clutter, no overloaded views, no hidden modes. Familiar patterns keep cognitive load low.
                </p>
              </article>
              <article class="m-card">
                <div style="width: 4rem; height: 4rem; border-radius: var(--radius-lg); background: var(--gradient-brand-subtle); display: grid; place-items: center; padding: 0.75rem;">
                  <img src="/icon-speed.svg" alt="" style="width: 100%; height: 100%;" />
                </div>
                <h3 class="m-card__title">Speed</h3>
                <p class="m-card__text">
                  Designed for real work. Fast startup, keyboard-first navigation, and reliable execution for long-running workflows.
                </p>
              </article>
              <article class="m-card">
                <div style="width: 4rem; height: 4rem; border-radius: var(--radius-lg); background: var(--gradient-brand-subtle); display: grid; place-items: center; padding: 0.75rem;">
                  <img src="/icon-consistency.svg" alt="" style="width: 100%; height: 100%;" />
                </div>
                <h3 class="m-card__title">Consistency</h3>
                <p class="m-card__text">
                  Learn Oatty once, use everywhere. The same command model powers CLI, TUI, workflows, and MCP tools across all vendors.
                </p>
              </article>
            </div>
          </div>
        </section>

        <!-- Features -->
        <section id="features" class="l-section">
          <div class="l-shell">
            <h2 style="font-size: var(--font-size-4xl); font-weight: 700; margin: 0 0 var(--space-xl);">
              What This Unlocks
            </h2>
            <div class="l-grid l-grid--three">
              <article class="m-card">
                <h3 class="m-card__title">Full API Coverage</h3>
                <p class="m-card__text">
                  Access the complete API surface without waiting for vendors to implement it in their CLI. If it's in the OpenAPI spec, it's available.
                </p>
              </article>
              <article class="m-card">
                <h3 class="m-card__title">Unified MCP Surface</h3>
                <p class="m-card__text">
                  One MCP server instead of fragmented plugin ecosystems. Extend functionality through the Model Context Protocol.
                </p>
              </article>
              <article class="m-card">
                <h3 class="m-card__title">Workflow Automation</h3>
                <p class="m-card__text">
                  Shareable, reviewable workflows instead of opaque scripts. Compose multi-vendor operations in a single file.
                </p>
              </article>
              <article class="m-card">
                <h3 class="m-card__title">Interactive TUI</h3>
                <p class="m-card__text">
                  Beautiful terminal UI that scales from quick commands to complex operations. Search, inspect, and execute with ease.
                </p>
              </article>
              <article class="m-card">
                <h3 class="m-card__title">Schema-Driven</h3>
                <p class="m-card__text">
                  Commands are automatically derived from OpenAPI documents. Stay in sync with API changes without manual updates.
                </p>
              </article>
              <article class="m-card">
                <h3 class="m-card__title">Secure by Default</h3>
                <p class="m-card__text">
                  Secrets are redacted from output and logs. Authentication is managed per-catalog with secure storage.
                </p>
              </article>
            </div>
          </div>
        </section>

        <!-- Installation -->
        <section id="install" class="l-section" style="background: var(--gradient-brand-subtle); border-radius: var(--radius-xl); padding: var(--space-4xl) 0;">
          <div class="l-shell">
            <h2 style="font-size: var(--font-size-4xl); font-weight: 700; margin: 0 0 var(--space-xl); text-align: center;">
              Getting Started
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
oatty catalog import ./schemas/your-api.json

# Search for commands
oatty search "create order"

# Run a workflow
oatty workflow list
oatty workflow run deploy --input env=staging</code></pre>
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
                <div class="m-logo" style="margin-bottom: var(--space-md);">
                  <img src="/logo-icon.svg" alt="Oatty logo" style="width: 2rem; height: 2rem;" />
                  <span style="font-size: var(--font-size-lg); font-weight: 700; letter-spacing: 0.05em;">OATTY</span>
                </div>
                <p style="color: var(--color-text-tertiary); font-size: var(--font-size-sm); margin: 0;">
                  Schema-driven CLI + TUI + MCP
                </p>
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
    `}}customElements.define("oatty-site-app",De);
