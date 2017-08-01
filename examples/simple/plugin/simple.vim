"

if !has('nvim') | finish | endif

if exists('g:simple_loaded')
  finish
endif
let g:simple_loaded = 1

let s:save_cpo = &cpo
set cpo&vim

let s:script_dir = expand('<sfile>:p:h')
function! s:RequireSimple(host) abort
  return jobstart([expand(s:script_dir . '../plugin.sh')], {'rpc': v:true})
endfunction

call remote#host#Register('simple', 'x', function("s:RequireSimple"))
call remote#host#RegisterPlugin('simple', '0', [
      \ { 'type': 'function', 'name': 'the_answer', 'sync': 1, 'opts': {}},
      \ ])

let &cpo = s:save_cpo
unlet s:save_cpo
