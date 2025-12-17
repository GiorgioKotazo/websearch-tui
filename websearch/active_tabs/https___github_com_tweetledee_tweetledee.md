# GitHub - tweetledee/tweetledee: A PHP library that provides an incredibly easy way to access Twitter data as JSON or RSS feed by URL or standard CLI syntax.

**Author**: tweetledee
**Source**: GitHub
**URL**: https://github.com/tweetledee/tweetledee

> A PHP library that provides an incredibly easy way to access Twitter data as JSON or RSS feed by URL or standard CLI syntax.   - GitHub - tweetledee/tweetledee: A PHP library that provides an incre...

---

## Tweetledee

[](#tweetledee)

**A PHP library that provides an incredibly easy way to access Twitter data as JSON, pretty printed JSON, or RSS feeds by URL or standard command line syntax. The Tweetledee files include caching to avoid exceeding the Twitter API v1.1 rate limits (see [caveats in the documentation](http://tweetledee.github.io/tweetledee/caching.html)!).**

## Obsolete

[](#obsolete)

As of May 2023 users report that the request on the v1 twitter API don't work anymore so Tweetledee is now dead.

## Documentation

[](#documentation)

- Docs Home: <http://tweetledee.github.io/tweetledee>
- Usage: <http://tweetledee.github.io/tweetledee/usage.html>
- Developer Docs: <http://tweetledee.github.io/tweetledee/developer.html>


## Current Release

[](#current-release)

- **0.5.3** : Fixed "user_timeline connection failure". Set Twitter username as author of a rss item.


## Next Release

[](#next-release)

- Use https for avatar img src.


## Changes

[](#changes)

- **0.5.2** : Quoted tweets can be displayed with a rl>=1 query parameter in the URL. All images for the tweet are loaded now.
- **0.5.1** : Improved hastag display.
- **0.5.0** : First release as the tweetledee organization. Thanks to Christopher Simpkins for encouraging other people to continue with the project. Now the full tweet is displayed following the 280 characters from twitter. Media is loaded as https. Added Some reference dockerfiles.
- **0.4.2** : added support for inline images in all RSS scripts [issue #51](https://github.com/tweetledee/tweetledee/issues/51). A big thanks to Vinh Nguyen for his pull request to add support for this feature!
- **0.4.1** : added support for JSON pretty printing in PHP versions 5.3+ (from 5.4+ previously) [issue #40](https://github.com/tweetledee/tweetledee/pull/40). Thanks much to Martín Lucas Golini @SpartanJ for his new pretty printing functions.
- **0.4.0** : added caching to all Tweetledee files with default 90 sec duration. This default cache interval can be changed with the `cache_interval` URL parameter (with a value in seconds). Great big thanks to Christian Varga (@levymetal) for his contributions to this update! The non-cached versions of the files from v0.3.6 have been renamed with an appended `_nocache` (e.g. `userrss.php` > `userrss_nocache.php`) for anyone who would like to implement their own caching.
- **0.3.6** : bug fix for multi-parameter search query exception bug [issue #30](https://github.com/tweetledee/tweetledee/issues/30). Thanks much for the issue report @adjeD!
- **0.3.5** : bug fix for Twitter search filters [issue #28](https://github.com/tweetledee/tweetledee/issues/28). Thanks much for the issue report @molis83!
- **0.3.4** : added Python and Ruby wrappers for the Tweetledee files
- **0.3.3** : bug fixes for [issue #15](https://github.com/tweetledee/tweetledee/issues/15) & [issue #16](https://github.com/tweetledee/tweetledee/issues/16). Thanks much for the contributions from @jjschwartz, @kabookey, and @mikeklimczak.
- **0.3.2** : bug fixes for [issue #14](https://github.com/tweetledee/tweetledee/issues/14)
- **0.3.1** : Updated all standard JSON files with cross site access to your Twitter JSON data from client side JavaScript code (sets the Access-Control-Allow-Origin header to accept all connections, i.e. cross origin resource sharing). Defaults to off. Set the flag `$TLD_JS = 1` in the file to activate this capability.
- **0.3.0** : You can now access Tweetledee from the command line locally or remotely via SSH and pipe the output to any application. Data is returned via the standard output stream when you access files with a terminal. Tweetledee will parse the parameters as standard command line switches. For single character parameters use short switches `-q` and for multiple character parameters use long switches `--user`.
- **0.2.9** : Added Twitter user lists RSS feeds `listsrss.php`, JSON `listsjson.php`, pretty printed JSON `listsjson_pp.php`


## The 1.5 Minute Guide to a Successful Install

[](#the-15-minute-guide-to-a-successful-install)

**You will need the following**:

- Access via URL: PHP version 5.3 or higher

- Access via command line: PHP version 5.3 or higher

- libcurl installed (provides cURL - <http://curl.haxx.se/libcurl/>)

- A [Twitter application account](https://dev.twitter.com/apps/new) from which you will obtain the:

  1. consumer key
  2. consumer secret
  3. access key
  4. access secret

### 3-Step Installation instructions:

[](#3-step-installation-instructions)

1. Open the file on the path tweetledee > tldlib > keys > tweetledee_keys.php in any text editor and enter the information that you obtained from your Twitter app in the corresponding fields. Leave the single quotes around the alphanumeric strings that you enter.

2. Upload the 'tweetledee' directory (that is located in the directory where this README file resides) to the public facing directory on your web server. On many servers, this is the public_html directory

3. Kick the tires with the following test (it gives you a user timeline RSS feed for your account):
http://[yourdomain]/tweetledee/userrss.php

That was easy... Go crazy, be good, have fun.

## What You Get

[](#what-you-get)

### Twitter RSS Feeds

[](#twitter-rss-feeds)

##### Favorites RSS Feed [`favoritesrss.php`] + [`favoritesrss_nocache.php`]

[](#favorites-rss-feed-favoritesrssphp--favoritesrss_nocachephp)

##### Home Timeline RSS Feed [`homerss.php`] + [`homerss_nocache.php`]

[](#home-timeline-rss-feed-homerssphp--homerss_nocachephp)

##### User Lists RSS Feed [`listsrss.php`] + [`listrss_nocache.php`]

[](#user-lists-rss-feed-listsrssphp--listrss_nocachephp)

##### User Timeline RSS Feed [`userrss.php`] + [`userrss_nocache.php`]

[](#user-timeline-rss-feed-userrssphp--userrss_nocachephp)

##### Search RSS Feed [`searchrss.php`] + [`searchrss_nocache.php`]

[](#search-rss-feed-searchrssphp--searchrss_nocachephp)

### Twitter JSON

[](#twitter-json)

##### Favorites JSON [`favoritesjson.php`] + [`favoritesjson_nocache.php`]

[](#favorites-json-favoritesjsonphp--favoritesjson_nocachephp)

##### Home Timeline JSON [`homejson.php`] + [`homejson_nocache.php`]

[](#home-timeline-json-homejsonphp--homejson_nocachephp)

##### User Lists JSON [`listsjson.php`] + [`listsjson_nocache.php`]

[](#user-lists-json-listsjsonphp--listsjson_nocachephp)

##### User Timeline JSON [`userjson.php`] + [`userjson_nocache.php`]

[](#user-timeline-json-userjsonphp--userjson_nocachephp)

##### Search JSON [`searchjson.php`] + + [`searchjson_nocache.php`]

[](#search-json-searchjsonphp---searchjson_nocachephp)

### Pretty Printed JSON

[](#pretty-printed-json)

##### Favorites Pretty Printed JSON [`favoritesjson_pp.php`] + [`favoritesjson_pp_nocache.php`]

[](#favorites-pretty-printed-json-favoritesjson_ppphp--favoritesjson_pp_nocachephp)

##### Home Timeline Pretty Printed JSON [`homejson_pp.php`] + [`homejson_pp_nocache.php`]

[](#home-timeline-pretty-printed-json-homejson_ppphp--homejson_pp_nocachephp)

##### User Lists Pretty Printed JSON [`listsjson_pp.php`] + [`listsjson_pp_nocache.php`]

[](#user-lists-pretty-printed-json-listsjson_ppphp--listsjson_pp_nocachephp)

##### User Timeline Pretty Printed JSON [`userjson_pp.php`] + [`userjson_pp_nocache.php`]

[](#user-timeline-pretty-printed-json-userjson_ppphp--userjson_pp_nocachephp)

##### Search Pretty Printed JSON [`searchjson_pp.php`] + [`searchjson_pp_nocache.php`]

[](#search-pretty-printed-json-searchjson_ppphp--searchjson_pp_nocachephp)

## Usage

[](#usage)

[Tweetledee Usage Examples](http://tweetledee.github.io/tweetledee/usage.html)

## Bugs & Questions

[](#bugs--questions)

If you find a bug, please post it as a new issue on the GitHub repository with [this information in your report](http://tweetledee.github.io/tweetledee/support.html#bug-reporting).

Looking for support? Check [this page](http://tweetledee.github.io/tweetledee/support.html).

## Contribute

[](#contribute)

If you would like to contribute to the project, have at it. [Fork the Tweetledee project](https://help.github.com/articles/fork-a-repo), include your changes, and [submit a pull request](https://help.github.com/articles/using-pull-requests) back to the main repository.

## License

[](#license)

MIT License - see the LICENSE.txt file in the source distribution
