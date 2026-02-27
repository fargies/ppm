To generate the gifs:

```bash
for f in *.cast ; do agg --no-loop --theme=github-dark "${f}" "${f/.cast/.gif}"; done
```
